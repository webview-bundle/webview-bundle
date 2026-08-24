import { assert, assertEquals, assertRejects, assertThrows } from '@std/assert';
import { decodeBase64 } from '@std/encoding/base64';
import { dirname } from '@std/path';
import {
  BundleBuilder,
  BundleProtocol,
  type BundleProtocolOptions,
  computeIntegrity,
  type HttpResponse,
  type IntegrityAlgorithm,
  type IntegrityPolicy,
  ProxyProtocol,
  parseIntegrity,
  readBundle,
  readBundleFromBytes,
  Source,
  type SourceIntegrityCheckMode,
  type SourceOptions,
  type UriPathResolver,
  WebviewBundleError,
  writeBundle,
  writeBundleToBytes,
} from './mod.ts';
import { buildBundleData, loadTestLib, tempDir, testSource } from './testing.ts';

loadTestLib();

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

Deno.test('BundleProtocol resolves an extensionless path with the html_extension resolver', async () => {
  using source = testSource();
  using protocol = new BundleProtocol(source, { pathResolver: 'html_extension' });
  // `/index` -> `/index.html`
  assertEquals((await protocol.handle('get', 'bundle://app/index')).status, 200);
  // The default (directory_index) would look for `/index/index.html` instead.
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
  assertThrows(() => new BundleProtocol(source, { pathResolver: 'nope' as UriPathResolver }));
  assertThrows(
    () =>
      new BundleProtocol(source, {
        // @ts-expect-error unknown bundle resolver discriminant
        bundleResolver: { type: 'nope' },
      })
  );
  // Options that are not an object would otherwise read as "no options" and serve with the defaults.
  assertThrows(
    () => new BundleProtocol(source, 'directory_index' as unknown as BundleProtocolOptions)
  );
});

Deno.test("the source's data-checksum options flow through what the protocol serves", async () => {
  using source = testSource();
  using protocol = new BundleProtocol(source);
  assertEquals((await protocol.handle('get', 'bundle://app/index.html')).status, 200);

  // The seed is part of the checksum, so a source configured with a seed the bundle was not packed
  // with mismatches when the protocol reads through it.
  using wrongSeedSource = testSource({ dataRead: { checksum: { seed: 1 } } });
  using wrongSeed = new BundleProtocol(wrongSeedSource);
  const error = await assertRejects(
    () => wrongSeed.handle('get', 'bundle://app/index.html'),
    WebviewBundleError
  );
  assertEquals(error.code, 'core.checksum_mismatch');

  using unverifiedSource = testSource({ dataRead: { checksum: { verify: false, seed: 1 } } });
  using unverified = new BundleProtocol(unverifiedSource);
  assertEquals((await unverified.handle('get', 'bundle://app/index.html')).status, 200);
});

Deno.test('Source accepts verification options and fails closed on a bad one', () => {
  {
    using _configured = testSource({
      integrity: { policy: 'optional', checkMode: 'only_remote' },
      dataRead: { checksum: { verify: true, seed: 0 } },
      headerRead: { checksum: { verify: true } },
      indexRead: { checksum: { verify: false, seed: 2 } },
      removeBundleChunkSize: 8,
    });
    using _off = testSource({ integrity: { policy: 'off' } });
  }
  const badMode = assertThrows(
    () => testSource({ integrity: { checkMode: 'sometimes' as SourceIntegrityCheckMode } }),
    WebviewBundleError
  );
  assertEquals(badMode.code, 'invalid_request');
  const badPolicy = assertThrows(
    // 'none' was the old spelling of 'off'; it must fail closed rather than pick a default.
    () => testSource({ integrity: { policy: 'none' as IntegrityPolicy } }),
    WebviewBundleError
  );
  assertEquals(badPolicy.code, 'invalid_request');
});

Deno.test('integrity options round-trip: strict + all rejects the unhashed builtin on load', async () => {
  // The builtin fixture manifest carries no integrity string, so `strict` must refuse to load it
  // when `checkMode: 'all'` selects builtin bundles — while the default `'only_remote'` mode
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
    // A dropped `dataRead.checksum.verify` would leave verification in a state the caller did not
    // ask for.
    () => testSource({ dataRead: { checksum: { verifyy: true } } } as unknown as SourceOptions),
    WebviewBundleError
  );
  assert(sourceError.message.includes('verifyy'), sourceError.message);

  const nestedError = assertThrows(
    // A dropped `integrity.checkMode` would leave builtin bundles unverified while the caller
    // believes they are covered.
    () => testSource({ integrity: { checkmode: 'all' } } as unknown as SourceOptions),
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

Deno.test('Source takes a data-checksum seed without disabling verification', () => {
  // The source verifies entry checksums by default; passing only the seed must keep it on (the
  // native default is pinned in `src/source.rs`, where the read options are observable).
  using seeded = testSource({ dataRead: { checksum: { seed: 1 } } });
  assert(seeded instanceof Source);
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

Deno.test('Source exposes source operations over the builtin fixture', async () => {
  using source = testSource();
  const app = (await source.listBundles()).find(b => b.item.name === 'app');
  assert(app != null, 'app bundle is listed');
  assertEquals(app.source, 'builtin');
  assertEquals(app.item.version, '1.0.0');
  assertEquals(app.item.status, 'current');

  assertEquals(await source.listRemoteBundles(), []);
  assertEquals(await source.getVersion('app'), { source: 'builtin', version: '1.0.0' });
  assertEquals(await source.getRemoteStagedVersion('app'), null);
  assertEquals(await source.getRemotePreviousVersion('app'), null);

  const path = await source.resolveFilepath('app');
  assert(path.endsWith('1.0.0.wvb'), path);
  assertEquals(source.getBuiltinBundleFilepath('app', '1.0.0'), path);

  // Version data exists (the manifest entry is present, even if empty).
  assert((await source.getBuiltinVersionData('app', '1.0.0')) != null);
  assertEquals(await source.getRemoteVersionData('app', '1.0.0'), null);

  // A descriptor that was never loaded → unload is a no-op.
  assertEquals(source.unload('app'), false);
});

Deno.test('Source stages, activates, removes and prunes a remote version', async () => {
  using source = testSource();
  const html = new TextEncoder().encode('<h1>app@2.0.0</h1>');
  await stageRemote(source, '2.0.0');

  assertEquals(await source.getRemoteStagedVersion('app'), '2.0.0');
  // Staged, not activated: the current version is still the builtin.
  assertEquals(await source.getVersion('app'), { source: 'builtin', version: '1.0.0' });

  assertEquals(await source.updateRemoteVersion('app', '2.0.0'), {
    name: 'app',
    version: '2.0.0',
    kind: 'settled',
  });
  assertEquals(await source.getVersion('app'), { source: 'remote', version: '2.0.0' });

  using fetched = await source.fetchRemoteBundle('app', '2.0.0');
  assertEquals(fetched.getData('/index.html'), html);

  // The version in use is kept unless the removal is forced.
  assertEquals((await source.removeRemoteBundle('app', '2.0.0')).kind, 'in_use');
  await stageRemote(source, '2.1.0');
  await source.updateRemoteVersion('app', '2.1.0');
  // 2.0.0 is now the previous version, so pruning leaves it in place.
  assertEquals(await source.pruneRemoteBundle('app'), { name: 'app', prunedVersions: [] });
  assertEquals((await source.removeRemoteBundle('app', '2.0.0', true)).kind, 'removed');
});

Deno.test('Source applies batched manifest operations', async () => {
  using source = testSource();
  await stageRemote(source, '2.0.0');
  await stageRemote(source, '2.1.0');

  assertEquals(await source.updateRemoteVersions({ app: '2.1.0' }), [
    { name: 'app', version: '2.1.0', kind: 'settled' },
  ]);
  assertEquals(await source.updateRemoteVersions({ nope: '1.0.0' }), [
    { name: 'nope', version: '1.0.0', kind: 'not_exists' },
  ]);
  assertEquals(await source.removeRemoteBundles({ app: { versions: ['2.0.0'] } }), [
    { name: 'app', version: '2.0.0', kind: 'removed' },
  ]);
  assertEquals(await source.pruneRemoteBundles(['app']), [{ name: 'app', prunedVersions: [] }]);
});

/** Writes a `.wvb` into the remote dir and records it in the manifest, without activating it. */
async function stageRemote(source: Source, version: string): Promise<void> {
  const filepath = source.getRemoteBundleFilepath('app', version);
  await Deno.mkdir(dirname(filepath), { recursive: true });
  await Deno.writeFile(filepath, buildBundleData('app', version));
  await source.stageRemoteBundle('app', { version });
}

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

  using dir = tempDir('wvb-deno-bundle-');
  const file = `${dir.path}/out.wvb`;
  const written = await writeBundle(bundle, file);
  assert(written > 0, 'writeBundle reports the bytes written');
  using read = await readBundle(file);
  assertEquals(read.getData('/style.css'), css);
  assertEquals(read.index()['/style.css'].contentType, 'text/css');
});

Deno.test('Source fetches the builtin bundle and reads it via descriptors', async () => {
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

  // load: remembers its own filepath, so getData takes only a path.
  using loaded = await source.load('app');
  assertEquals(await loaded.getData(path), data);
  assertEquals(loaded.header().version, 'v1');
  // The descriptor is cached now, so unload reports it removed one.
  assertEquals(source.unload('app'), true);
});
