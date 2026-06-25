import { assert, assertEquals } from '@std/assert';
import { fromFileUrl } from '@std/path';
import { loadLib } from '@wvb/deno';
import { bundleProtocol, webviewBundle } from '../mod.ts';

// Use the locally-built cdylib + the committed ffi builtin fixture (bundle "app" v1.0.0).
const ext = Deno.build.os === 'windows' ? 'dll' : Deno.build.os === 'darwin' ? 'dylib' : 'so';
const prefix = Deno.build.os === 'windows' ? '' : 'lib';
const DYLIB = fromFileUrl(
  new URL(`../../../target/debug/${prefix}wvb_deno.${ext}`, import.meta.url)
);
const BUILTIN_DIR = fromFileUrl(
  new URL('../../ffi/apple/ios/assets/bundles/builtin', import.meta.url)
);

// Load the native library once at module top level — not inside a test — so Deno's per-test
// resource-leak sanitizer doesn't flag the intentionally process-lifetime FFI handle.
loadLib(DYLIB);

function makeApp() {
  return webviewBundle({
    source: { builtinDir: BUILTIN_DIR, remoteDir: Deno.makeTempDirSync({ prefix: 'wvb-test-' }) },
    protocols: [bundleProtocol('app')],
  });
}

Deno.test('webviewBundle.fetch serves the builtin bundle index at the root', async () => {
  const app = makeApp();
  const res = await app.fetch(new Request('http://127.0.0.1/'));
  assertEquals(res.status, 200);
  assertEquals(res.headers.get('content-type'), 'text/html');
  assert((await res.text()).includes('Pagination with SSG'));
});

Deno.test('webviewBundle.fetch forwards Range (206) and returns 404', async () => {
  const app = makeApp();

  const ranged = await app.fetch(
    new Request('http://127.0.0.1/build.png', { headers: { Range: 'bytes=0-99' } })
  );
  assertEquals(ranged.status, 206);
  assertEquals((await ranged.arrayBuffer()).byteLength, 100);

  const missing = await app.fetch(new Request('http://127.0.0.1/nope'));
  assertEquals(missing.status, 404);
});

Deno.test('webviewBundle exposes source + protocolSchemes', () => {
  const app = makeApp();
  assert(app.source != null);
  assertEquals(app.protocolSchemes, ['app']);
  assertEquals(app.remote, null);
  assertEquals(app.updater, null);
});
