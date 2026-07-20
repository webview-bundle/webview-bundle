import { assert, assertEquals, assertRejects, assertThrows } from '@std/assert';
import { decodeBase64 } from '@std/encoding/base64';
import { fromFileUrl } from '@std/path';
import {
  BundleBuilder,
  BundleProtocol,
  type BundleProtocolOptions,
  BundleSource,
  type BundleSourceConfig,
  type BundleSourceVerifyMode,
  computeIntegrity,
  type HttpResponse,
  type IntegrityAlgorithm,
  type IntegrityPolicy,
  loadLib,
  type PathResolver,
  ProxyProtocol,
  parseIntegrity,
  Remote,
  readBundle,
  readBundleFromBytes,
  Updater,
  WebviewBundleError,
  writeBundle,
  writeBundleToBytes,
} from './mod.ts';

// Resolve the locally-built cdylib and the committed builtin fixture (bundle "app" v1.0.0).
const ext = Deno.build.os === 'windows' ? 'dll' : Deno.build.os === 'darwin' ? 'dylib' : 'so';
const prefix = Deno.build.os === 'windows' ? '' : 'lib';
const DYLIB = fromFileUrl(
  new URL(`../../../target/release/${prefix}wvb_deno.${ext}`, import.meta.url)
);
const BUILTIN_DIR = fromFileUrl(new URL('../fixtures/builtin', import.meta.url));

loadLib(DYLIB);

/**
 * A {@link BundleSource} over the builtin fixture, backed by a temp remote dir. Disposing it frees
 * the handle and removes the temp dir, so tests only need `using source = testSource()`.
 */
function testSource(
  options: Omit<BundleSourceConfig, 'builtinDir' | 'remoteDir'> = {}
): BundleSource {
  const remoteDir = Deno.makeTempDirSync({ prefix: 'wvb-deno-test-' });
  const removeRemoteDir = () => Deno.removeSync(remoteDir, { recursive: true });
  try {
    Deno.writeTextFileSync(
      `${remoteDir}/manifest.json`,
      JSON.stringify({ manifestVersion: 1, entries: {} })
    );
    const source = new BundleSource({ builtinDir: BUILTIN_DIR, remoteDir, ...options });
    return Object.assign(source, {
      [Symbol.dispose]: () => {
        try {
          source.free();
        } finally {
          removeRemoteDir();
        }
      },
    });
  } catch (e) {
    // The source never took ownership of the temp dir, so nothing else will remove it.
    removeRemoteDir();
    throw e;
  }
}

Deno.test('BundleProtocol serves the builtin bundle index via directory-index', async () => {
  using source = testSource();
  using protocol = new BundleProtocol(source);
  const res = await protocol.handle('get', 'bundle://app/');
  assertEquals(res.status, 200);
  assertEquals(res.headers['content-type'], 'text/html');
  assert(new TextDecoder().decode(res.body).includes('Pagination with SSG'));
});

Deno.test('BundleProtocol serves a nested path and reports content-type', async () => {
  using source = testSource();
  using protocol = new BundleProtocol(source);
  const res = await protocol.handle('get', 'bundle://app/category/2/index.html');
  assertEquals(res.status, 200);
  assertEquals(res.headers['content-type'], 'text/html');
});

Deno.test('BundleProtocol honors HTTP Range with 206', async () => {
  using source = testSource();
  using protocol = new BundleProtocol(source);
  const res = await protocol.handle('get', 'bundle://app/build.png', { Range: 'bytes=0-99' });
  assertEquals(res.status, 206);
  assertEquals(res.body.length, 100);
  assert(res.headers['content-range']?.startsWith('bytes 0-99/'));
});

Deno.test('BundleProtocol returns 404 for a missing path and 405 for POST', async () => {
  using source = testSource();
  using protocol = new BundleProtocol(source);
  assertEquals((await protocol.handle('get', 'bundle://app/nope.html')).status, 404);
  assertEquals((await protocol.handle('post', 'bundle://app/')).status, 405);
});

// The adapter each host writes over the binding's `HttpResponse` (see `@wvb/deno-desktop`).
function toResponse(res: HttpResponse): Response {
  const headers = new Headers();
  for (const [name, value] of Object.entries(res.headers)) {
    headers.set(name, value);
  }
  return new Response(res.body, { status: res.status, headers });
}

Deno.test('an HttpResponse carries the status, headers and body of a web Response', async () => {
  using source = testSource();
  using protocol = new BundleProtocol(source);
  const res = toResponse(await protocol.handle('get', 'bundle://app/index.html'));
  assertEquals(res.status, 200);
  assertEquals(res.headers.get('content-type'), 'text/html');
  assert((await res.text()).includes('Pagination with SSG'));
});

Deno.test('BundleProtocol resolves an extensionless path with the htmlExtension resolver', async () => {
  using source = testSource();
  using protocol = new BundleProtocol(source, { pathResolver: 'htmlExtension' });
  // `/index` -> `/index.html`
  assertEquals((await protocol.handle('get', 'bundle://app/index')).status, 200);
  // The default (directoryIndex) would look for `/index/index.html` instead.
  using byDirectory = new BundleProtocol(source);
  assertEquals((await byDirectory.handle('get', 'bundle://app/index')).status, 404);
});

Deno.test('BundleProtocol does not rewrite the path with the exact resolver', async () => {
  using source = testSource();
  using protocol = new BundleProtocol(source, { pathResolver: 'exact' });
  assertEquals((await protocol.handle('get', 'bundle://app/index.html')).status, 200);
  assertEquals((await protocol.handle('get', 'bundle://app/')).status, 404);
});

Deno.test('BundleProtocol resolves the bundle name from a path segment', async () => {
  using source = testSource();
  using protocol = new BundleProtocol(source, { bundleResolver: { type: 'pathname' } });
  // Bundle "app" resolves; the path keeps the segment naming it, so the entry is missing (404).
  assertEquals((await protocol.handle('get', 'bundle://cdn/app/index.html')).status, 404);
  // An unknown bundle name fails the request instead of answering it.
  const error = await assertRejects(
    () => protocol.handle('get', 'bundle://cdn/nope/index.html'),
    WebviewBundleError
  );
  assertEquals(error.code, 'core.bundle_not_found');
});

Deno.test('BundleProtocol rejects an unknown resolver option (fails closed)', () => {
  using source = testSource();
  assertThrows(() => new BundleProtocol(source, { pathResolver: 'nope' as PathResolver }));
  assertThrows(
    () =>
      new BundleProtocol(source, {
        // @ts-expect-error unknown bundle resolver discriminant
        bundleResolver: { type: 'nope' },
      })
  );
  // Options that are not an object would otherwise read as "no options" and serve with the defaults.
  assertThrows(
    () => new BundleProtocol(source, 'directoryIndex' as unknown as BundleProtocolOptions)
  );
});

Deno.test("the source's data-checksum options flow through what the protocol serves", async () => {
  using source = testSource();
  using protocol = new BundleProtocol(source);
  assertEquals((await protocol.handle('get', 'bundle://app/index.html')).status, 200);

  // The seed is part of the checksum, so a source configured with a seed the bundle was not packed
  // with mismatches when the protocol reads through it.
  using wrongSeedSource = testSource({ dataReadOptions: { checksum: { seed: 1 } } });
  using wrongSeed = new BundleProtocol(wrongSeedSource);
  const error = await assertRejects(
    () => wrongSeed.handle('get', 'bundle://app/index.html'),
    WebviewBundleError
  );
  assertEquals(error.code, 'core.checksum_mismatch');

  using unverifiedSource = testSource({
    dataReadOptions: { checksum: { verify: false, seed: 1 } },
  });
  using unverified = new BundleProtocol(unverifiedSource);
  assertEquals((await unverified.handle('get', 'bundle://app/index.html')).status, 200);
});

Deno.test('BundleSource accepts verification options and fails closed on a bad one', () => {
  {
    using _configured = testSource({
      integrity: { policy: 'optional', checkMode: 'onlyRemote' },
      dataReadOptions: { checksum: { verify: true, seed: 0 } },
      headerReadOptions: { checksum: { verify: true } },
      indexReadOptions: { checksum: { verify: false, seed: 2 } },
    });
    using _off = testSource({ integrity: { policy: 'off' }, signature: { verifyMode: 'all' } });
  }
  const badMode = assertThrows(
    () => testSource({ integrity: { checkMode: 'sometimes' as BundleSourceVerifyMode } }),
    WebviewBundleError
  );
  // No verifier was given, so this is not a key failure.
  assertEquals(badMode.code, 'unknown');
  const badPolicy = assertThrows(
    // 'none' was the old spelling of 'off'; it must fail closed rather than pick a default.
    () => testSource({ integrity: { policy: 'none' as IntegrityPolicy } }),
    WebviewBundleError
  );
  assertEquals(badPolicy.code, 'unknown');
  const badKey = assertThrows(
    () =>
      testSource({
        // A key too short to be an ed25519 public key: the source must not fall back to unverified.
        signature: { verify: { algorithm: 'ed25519', key: { format: 'raw', data: 'AAAA' } } },
      }),
    WebviewBundleError
  );
  assertEquals(badKey.code, 'invalid_signature_options');
});

Deno.test('integrity options round-trip: strict + all rejects the unhashed builtin on load', async () => {
  // The builtin fixture manifest carries no integrity string, so `strict` must refuse to load it
  // when `checkMode: 'all'` selects builtin bundles — while the default `'onlyRemote'` mode
  // leaves them alone.
  using strict = testSource({ integrity: { policy: 'strict', checkMode: 'all' } });
  using strictProtocol = new BundleProtocol(strict);
  const error = await assertRejects(
    () => strictProtocol.handle('get', 'bundle://app/index.html'),
    WebviewBundleError
  );
  assertEquals(error.code, 'core.integrity_verify_failed');

  using remoteOnly = testSource({ integrity: { policy: 'strict' } });
  using protocol = new BundleProtocol(remoteOnly);
  assertEquals((await protocol.handle('get', 'bundle://app/index.html')).status, 200);
});

Deno.test('a misspelled option is rejected instead of silently ignored', () => {
  const sourceError = assertThrows(
    // A dropped `dataReadOptions.checksum.verify` would leave verification in a state the caller did
    // not ask for.
    () =>
      testSource({
        dataReadOptions: { checksum: { verifyy: true } },
      } as unknown as BundleSourceConfig),
    WebviewBundleError
  );
  assert(sourceError.message.includes('verifyy'), sourceError.message);

  const nestedError = assertThrows(
    // A dropped `integrity.checkMode` would leave builtin bundles unverified while the caller
    // believes they are covered.
    () => testSource({ integrity: { checkmode: 'all' } } as unknown as BundleSourceConfig),
    WebviewBundleError
  );
  assert(nestedError.message.includes('checkmode'), nestedError.message);

  using source = testSource();
  const protocolError = assertThrows(
    () =>
      new BundleProtocol(source, {
        verifyChecksum: false,
      } as unknown as BundleProtocolOptions),
    WebviewBundleError
  );
  assert(protocolError.message.includes('verifyChecksum'), protocolError.message);
});

Deno.test('BundleSource takes a data-checksum seed without disabling verification', () => {
  // The source verifies entry checksums by default; passing only the seed must keep it on (the
  // native default is pinned in `src/lib.rs`, where the read options are observable).
  using seeded = testSource({ dataReadOptions: { checksum: { seed: 1 } } });
  assert(seeded instanceof BundleSource);
});

Deno.test('ProxyProtocol constructs and is disposable', () => {
  using proxy = new ProxyProtocol({ app: 'http://localhost:5173' });
  assert(proxy instanceof ProxyProtocol);
});

Deno.test('ProxyProtocol fails on a host that is not mapped', async () => {
  using proxy = new ProxyProtocol({ app: 'http://localhost:5173' });
  const error = await assertRejects(
    () => proxy.handle('get', 'app://other/index.html'),
    WebviewBundleError
  );
  assertEquals(error.code, 'core.cannot_resolve_proxy_server');
});

Deno.test('Remote constructs and rejects (error path through FFI) on an unreachable endpoint', async () => {
  using remote = new Remote('http://127.0.0.1:59999', { http: { connectTimeout: 2000 } });
  await assertRejects(() => remote.listBundles());
});

Deno.test('Updater constructs with a source + remote and propagates errors', async () => {
  using source = testSource();
  using remote = new Remote('http://127.0.0.1:59999', { http: { connectTimeout: 2000 } });
  using updater = new Updater(source, remote, { channel: 'stable', integrityPolicy: 'strict' });
  assert(updater instanceof Updater);
  await assertRejects(() => updater.listRemotes());
});

Deno.test('Updater fails closed on an unknown integrityPolicy value', () => {
  using source = testSource();
  using remote = new Remote('http://127.0.0.1:59999', { http: { connectTimeout: 2000 } });
  // 'none' was the old spelling of 'off'; it must fail construction rather than be ignored.
  assertThrows(
    () => new Updater(source, remote, { integrityPolicy: 'none' as IntegrityPolicy }),
    WebviewBundleError
  );
});

Deno.test('BundleSource exposes source operations over the builtin fixture', async () => {
  using source = testSource();
  const app = (await source.listBundles()).find(b => b.name === 'app');
  assert(app != null, 'app bundle is listed');
  assertEquals(app.type, 'builtin');
  assertEquals(app.version, '1.0.0');
  assertEquals(app.current, true);

  assertEquals(await source.loadVersion('app'), { type: 'builtin', version: '1.0.0' });

  const path = await source.resolveFilepath('app');
  assert(path.endsWith('app_1.0.0.wvb'), path);
  assertEquals(source.getBuiltinBundleFilepath('app', '1.0.0'), path);

  // Metadata exists (the manifest entry is present, even if empty).
  assert((await source.loadBuiltinMetadata('app', '1.0.0')) != null);

  // A descriptor that was never loaded → unload is a no-op.
  assertEquals(source.unloadDescriptor('app'), false);
});

// A valid Ed25519 SPKI public key (PEM) for exercising the signatureVerifier wiring.
const ED25519_PUBLIC_PEM = `-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAzUROGx/OqiO9ZwxWsaG3ChmBqEGpXKTC9DmAVx86J5E=
-----END PUBLIC KEY-----`;

Deno.test('Updater accepts a declarative ed25519 signatureVerifier (PEM text + DER bytes)', () => {
  using source = testSource();
  using remote = new Remote('http://127.0.0.1:59999', { http: { connectTimeout: 2000 } });
  using pem = new Updater(source, remote, {
    signatureVerifier: {
      algorithm: 'ed25519',
      key: { format: 'spkiPem', data: ED25519_PUBLIC_PEM },
    },
  });
  assert(pem instanceof Updater);
  // Same key as raw DER bytes — exercises the Uint8Array → base64 → FFI base64-decode path.
  const der = decodeBase64(
    ED25519_PUBLIC_PEM.split('\n')
      .filter(line => !line.startsWith('-'))
      .join('')
  );
  using derUpdater = new Updater(source, remote, {
    signatureVerifier: { algorithm: 'ed25519', key: { format: 'spkiDer', data: der } },
  });
  assert(derUpdater instanceof Updater);
});

Deno.test('Updater fails closed on an invalid signatureVerifier key', () => {
  using source = testSource();
  using remote = new Remote('http://127.0.0.1:59999', { http: { connectTimeout: 2000 } });
  assertThrows(
    () =>
      new Updater(source, remote, {
        signatureVerifier: {
          algorithm: 'ed25519',
          key: { format: 'spkiPem', data: 'not a valid key' },
        },
      })
  );
});

Deno.test('Remote sends the configured http options on every request', async () => {
  let seen: Headers | undefined;
  const server = Deno.serve({ port: 0, onListen: () => {} }, req => {
    seen = req.headers;
    return Response.json([{ name: 'app', version: '1.0.0' }]);
  });
  try {
    using remote = new Remote(`http://127.0.0.1:${server.addr.port}`, {
      http: {
        defaultHeaders: { authorization: 'Bearer tok-123', 'x-tenant': 'acme' },
        userAgent: 'wvb-deno-test/1.0',
      },
    });
    await remote.listBundles();

    assertEquals(seen?.get('authorization'), 'Bearer tok-123');
    assertEquals(seen?.get('x-tenant'), 'acme');
    assertEquals(seen?.get('user-agent'), 'wvb-deno-test/1.0');
  } finally {
    await server.shutdown();
  }
});

Deno.test('computeIntegrity produces the string core produces', () => {
  // Same vector as core's own integrity_serialize test.
  const integrity = computeIntegrity('sha256', new TextEncoder().encode('test'));
  assertEquals(integrity.serialize(), 'sha256:n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg=');
  assertEquals(integrity.value(), decodeBase64('n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg='));
});

Deno.test('computeIntegrity digests each algorithm to its own width', () => {
  const data = new TextEncoder().encode('<h1>hello</h1>');
  const widths: Record<IntegrityAlgorithm, number> = { sha256: 32, sha384: 48, sha512: 64 };
  for (const [algorithm, width] of Object.entries(widths) as [IntegrityAlgorithm, number][]) {
    const integrity = computeIntegrity(algorithm, data);
    assert(integrity.serialize().startsWith(`${algorithm}:`));
    assertEquals(integrity.value().length, width);
  }
});

Deno.test('parseIntegrity round-trips and validates the right bytes only', () => {
  const data = new TextEncoder().encode('<h1>hello</h1>');
  const serialized = computeIntegrity('sha384', data).serialize();
  const parsed = parseIntegrity(serialized);

  assertEquals(parsed.serialize(), serialized);
  assertEquals(parsed.value(), computeIntegrity('sha384', data).value());
  assert(parsed.validate(data));
  assert(!parsed.validate(new TextEncoder().encode('tampered')));
});

Deno.test('parseIntegrity rejects a malformed string with the shared error code', () => {
  const error = assertThrows(() => parseIntegrity('not-an-integrity'));
  assert(error instanceof WebviewBundleError);
  assertEquals(error.code, 'core.invalid_integrity');
});

Deno.test('BundleBuilder builds a bundle and round-trips through bytes', () => {
  using builder = new BundleBuilder();
  const html = new TextEncoder().encode('<html>hi</html>');
  const js = new TextEncoder().encode('console.log(1)');
  // insertEntry returns false when newly added, true when it replaced an existing entry.
  assertEquals(builder.insertEntry('/index.html', html), false);
  assertEquals(builder.insertEntry('/app.js', js), false);
  assertEquals(builder.insertEntry('/index.html', html), true);
  assert(builder.containsEntry('/index.html'));
  assertEquals(builder.entryPaths().toSorted(), ['/app.js', '/index.html']);
  assertEquals(builder.removeEntry('/app.js'), true);
  assert(!builder.containsEntry('/app.js'));

  using bundle = builder.build();
  assertEquals(bundle.getData('/index.html'), html);
  assertEquals(bundle.getData('/missing'), null);
  assertEquals(bundle.getDataChecksum('/missing'), null);
  assert(typeof bundle.getDataChecksum('/index.html') === 'number');

  const header = bundle.header();
  assertEquals(header.version, 'v1');
  const index = bundle.index();
  assertEquals(Object.keys(index), ['/index.html']);
  assertEquals(index['/index.html'].contentType, 'text/html');
  assertEquals(index['/index.html'].contentLength, html.byteLength);

  // Serialize → reparse: the round-tripped bundle serves the same bytes.
  const bytes = writeBundleToBytes(bundle);
  using reparsed = readBundleFromBytes(bytes);
  assertEquals(reparsed.getData('/index.html'), html);
});

Deno.test('writeBundle and readBundle round-trip through a file', async () => {
  using builder = new BundleBuilder();
  const css = new TextEncoder().encode('body { color: red }');
  builder.insertEntry('/style.css', css);
  using bundle = builder.build();

  const dir = await Deno.makeTempDir({ prefix: 'wvb-deno-bundle-' });
  try {
    const file = `${dir}/out.wvb`;
    const written = await writeBundle(bundle, file);
    assert(written > 0, 'writeBundle reports the bytes written');
    using read = await readBundle(file);
    assertEquals(read.getData('/style.css'), css);
    assertEquals(read.index()['/style.css'].contentType, 'text/css');
  } finally {
    await Deno.remove(dir, { recursive: true });
  }
});

Deno.test('BundleSource fetches the builtin bundle and reads it via descriptors', async () => {
  using source = testSource();
  using bundle = await source.fetchBundle('app');
  const paths = Object.keys(bundle.index());
  assert(paths.length > 0, 'the builtin app bundle has entries');
  const path = paths[0];
  const data = bundle.getData(path);
  assert(data != null, 'the entry has data');

  // fetchDescriptor: metadata only; entry reads reopen the file at `filepath`.
  const filepath = await source.resolveFilepath('app');
  using descriptor = await source.fetchDescriptor('app');
  assertEquals(Object.keys(descriptor.index()).toSorted(), paths.toSorted());
  assertEquals(await descriptor.getData(filepath, path), data);
  assertEquals(await descriptor.getData(filepath, '/nope'), null);

  // loadDescriptor: remembers its own filepath, so getData takes only a path.
  using loaded = await source.loadDescriptor('app');
  assertEquals(await loaded.getData(path), data);
  assertEquals(loaded.header().version, 'v1');
  // The descriptor is cached now, so unload reports it removed one.
  assertEquals(source.unloadDescriptor('app'), true);
});

Deno.test('writeRemoteBundleData stages a downloaded bundle for activation', async () => {
  using source = testSource();
  using builder = new BundleBuilder();
  const html = new TextEncoder().encode('<h1>v2</h1>');
  builder.insertEntry('/index.html', html);
  using bundle = builder.build();
  const bytes = writeBundleToBytes(bundle);

  await source.writeRemoteBundleData('app', '2.0.0', bytes);
  // Staged, not activated: the current version is still the builtin.
  assertEquals(await source.loadVersion('app'), { type: 'builtin', version: '1.0.0' });
  await source.updateRemoteVersion('app', '2.0.0');
  assertEquals(await source.loadVersion('app'), { type: 'remote', version: '2.0.0' });

  using fetched = await source.fetchRemoteBundle('app', '2.0.0');
  assertEquals(fetched.getData('/index.html'), html);
});
