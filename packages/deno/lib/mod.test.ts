import { assert, assertEquals, assertRejects, assertThrows } from '@std/assert';
import { decodeBase64 } from '@std/encoding/base64';
import { fromFileUrl } from '@std/path';
import {
  BundleProtocol,
  type BundleProtocolOptions,
  BundleSource,
  type HttpResponse,
  loadLib,
  type PathResolver,
  ProxyProtocol,
  type ProxyProtocolOptions,
  Remote,
  Updater,
  WebviewBundleError,
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
function testSource(): BundleSource {
  const remoteDir = Deno.makeTempDirSync({ prefix: 'wvb-deno-test-' });
  const removeRemoteDir = () => Deno.removeSync(remoteDir, { recursive: true });
  try {
    Deno.writeTextFileSync(
      `${remoteDir}/manifest.json`,
      JSON.stringify({ manifestVersion: 1, entries: {} })
    );
    const source = new BundleSource({ builtinDir: BUILTIN_DIR, remoteDir });
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

Deno.test('ProxyProtocol constructs and is disposable', () => {
  using proxy = new ProxyProtocol({ app: 'http://localhost:5173' });
  assert(proxy instanceof ProxyProtocol);

  using withOptions = new ProxyProtocol({ app: 'http://localhost:5173' }, { maxCacheBytes: 0 });
  assert(withOptions instanceof ProxyProtocol);
});

Deno.test('ProxyProtocol rejects an unknown option value (fails closed)', () => {
  assertThrows(
    () =>
      new ProxyProtocol(
        { app: 'http://localhost:5173' },
        { maxCacheBytes: 'lots' as unknown as number }
      )
  );
  assertThrows(
    () => new ProxyProtocol({ app: 'http://localhost:5173' }, 0 as unknown as ProxyProtocolOptions)
  );
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
