import { assert, assertEquals, assertRejects, assertThrows } from '@std/assert';
import { decodeBase64 } from '@std/encoding/base64';
import { fromFileUrl } from '@std/path';
import {
  BundleProtocol,
  BundleSource,
  LocalProtocol,
  loadLib,
  Remote,
  toResponse,
  Updater,
} from './mod.ts';

// Resolve the locally-built cdylib and the committed builtin fixture (bundle "app" v1.0.0).
const ext = Deno.build.os === 'windows' ? 'dll' : Deno.build.os === 'darwin' ? 'dylib' : 'so';
const prefix = Deno.build.os === 'windows' ? '' : 'lib';
const DYLIB = fromFileUrl(
  new URL(`../../../target/debug/${prefix}wvb_deno.${ext}`, import.meta.url)
);
const BUILTIN_DIR = fromFileUrl(new URL('../fixtures/builtin', import.meta.url));

loadLib(DYLIB);

async function withSource(fn: (source: BundleSource) => Promise<void>): Promise<void> {
  const remoteDir = await Deno.makeTempDir({ prefix: 'wvb-deno-test-' });
  await Deno.writeTextFile(
    `${remoteDir}/manifest.json`,
    JSON.stringify({ manifestVersion: 1, entries: {} })
  );
  using source = new BundleSource({ builtinDir: BUILTIN_DIR, remoteDir });
  try {
    await fn(source);
  } finally {
    await Deno.remove(remoteDir, { recursive: true });
  }
}

Deno.test('BundleProtocol serves the builtin bundle index via directory-index', async () => {
  await withSource(async source => {
    using protocol = new BundleProtocol(source);
    const res = await protocol.handle('get', 'bundle://app/');
    assertEquals(res.status, 200);
    assertEquals(res.headers['content-type'], 'text/html');
    assert(new TextDecoder().decode(res.body).includes('Pagination with SSG'));
  });
});

Deno.test('BundleProtocol serves a nested path and reports content-type', async () => {
  await withSource(async source => {
    using protocol = new BundleProtocol(source);
    const res = await protocol.handle('get', 'bundle://app/category/2/index.html');
    assertEquals(res.status, 200);
    assertEquals(res.headers['content-type'], 'text/html');
  });
});

Deno.test('BundleProtocol honors HTTP Range with 206', async () => {
  await withSource(async source => {
    using protocol = new BundleProtocol(source);
    const res = await protocol.handle('get', 'bundle://app/build.png', { Range: 'bytes=0-99' });
    assertEquals(res.status, 206);
    assertEquals(res.body.length, 100);
    assert(res.headers['content-range']?.startsWith('bytes 0-99/'));
  });
});

Deno.test('BundleProtocol returns 404 for a missing path and 405 for POST', async () => {
  await withSource(async source => {
    using protocol = new BundleProtocol(source);
    assertEquals((await protocol.handle('get', 'bundle://app/nope.html')).status, 404);
    assertEquals((await protocol.handle('post', 'bundle://app/')).status, 405);
  });
});

Deno.test('toResponse converts an HttpResponse to a web Response', async () => {
  await withSource(async source => {
    using protocol = new BundleProtocol(source);
    const res = toResponse(await protocol.handle('get', 'bundle://app/index.html'));
    assertEquals(res.status, 200);
    assertEquals(res.headers.get('content-type'), 'text/html');
    assert((await res.text()).includes('Pagination with SSG'));
  });
});

Deno.test('LocalProtocol constructs and is disposable', () => {
  using local = new LocalProtocol({ app: 'http://localhost:5173' });
  assert(local instanceof LocalProtocol);
});

Deno.test('Remote constructs and rejects (error path through FFI) on an unreachable endpoint', async () => {
  using remote = new Remote('http://127.0.0.1:59999', { http: { connectTimeout: 2000 } });
  await assertRejects(() => remote.listBundles());
});

Deno.test('Updater constructs with a source + remote and propagates errors', async () => {
  await withSource(async source => {
    using remote = new Remote('http://127.0.0.1:59999', { http: { connectTimeout: 2000 } });
    using updater = new Updater(source, remote, { channel: 'stable', integrityPolicy: 'strict' });
    assert(updater instanceof Updater);
    await assertRejects(() => updater.listRemotes());
  });
});

Deno.test('BundleSource exposes source operations over the builtin fixture', async () => {
  await withSource(async source => {
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
});

// A valid Ed25519 SPKI public key (PEM) for exercising the signatureVerifier wiring.
const ED25519_PUBLIC_PEM = `-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAzUROGx/OqiO9ZwxWsaG3ChmBqEGpXKTC9DmAVx86J5E=
-----END PUBLIC KEY-----`;

Deno.test('Updater accepts a declarative ed25519 signatureVerifier (PEM text + DER bytes)', async () => {
  await withSource(async source => {
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
});

Deno.test('Updater fails closed on an invalid signatureVerifier key', async () => {
  await withSource(async source => {
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
});
