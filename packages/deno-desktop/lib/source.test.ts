import { assert, assertEquals } from '@std/assert';
import { fromFileUrl } from '@std/path';
import { loadLib } from '@wvb/deno';
import { appDataDir, bundleSource } from './mod.ts';

const ext = Deno.build.os === 'windows' ? 'dll' : Deno.build.os === 'darwin' ? 'dylib' : 'so';
const prefix = Deno.build.os === 'windows' ? '' : 'lib';
const DYLIB = fromFileUrl(
  new URL(`../../../target/debug/${prefix}wvb_deno.${ext}`, import.meta.url)
);
const BUILTIN_DIR = fromFileUrl(
  new URL('../../ffi/apple/ios/assets/bundles/builtin', import.meta.url)
);

loadLib(DYLIB);

Deno.test('appDataDir honors the WVB_APP_DATA_DIR override', () => {
  Deno.env.set('WVB_APP_DATA_DIR', '/tmp/wvb-appdata-override');
  try {
    assertEquals(appDataDir(), '/tmp/wvb-appdata-override');
  } finally {
    Deno.env.delete('WVB_APP_DATA_DIR');
  }
});

Deno.test('appDataDir resolves the OS application-data directory by default', () => {
  const dir = appDataDir();
  assert(dir.length > 0);
  if (Deno.build.os === 'darwin') {
    assert(dir.endsWith('Library/Application Support'), dir);
  } else if (Deno.build.os === 'linux') {
    assert(dir.includes('.local/share') || dir.includes('/'), dir);
  }
});

Deno.test('bundleSource writes the remote dir under the app-data directory', () => {
  const base = Deno.makeTempDirSync({ prefix: 'wvb-appdata-' });
  Deno.env.set('WVB_APP_DATA_DIR', base);
  try {
    using _src = bundleSource({ builtinDir: BUILTIN_DIR, appName: 'myapp' });
    // remoteDir defaults to <app-data>/<appName>/bundles and is seeded with an empty manifest.
    assertEquals(Deno.statSync(`${base}/myapp/bundles/manifest.json`).isFile, true);
  } finally {
    Deno.env.delete('WVB_APP_DATA_DIR');
  }
});
