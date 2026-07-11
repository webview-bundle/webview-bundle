import { assert, assertEquals } from '@std/assert';
import { fromFileUrl } from '@std/path';
import { loadLib } from '@wvb/deno';
import {
  type DenoBrowserWindow,
  dispatch,
  INVOKE_BINDING,
  registerBindings,
  type WebviewBundle,
  webviewBundle,
} from './mod.ts';

const ext = Deno.build.os === 'windows' ? 'dll' : Deno.build.os === 'darwin' ? 'dylib' : 'so';
const prefix = Deno.build.os === 'windows' ? '' : 'lib';
const DYLIB = fromFileUrl(
  new URL(`../../../target/release/${prefix}wvb_deno.${ext}`, import.meta.url)
);
const BUILTIN_DIR = fromFileUrl(new URL('../fixtures/builtin', import.meta.url));

loadLib(DYLIB);

function makeApp(): WebviewBundle {
  return webviewBundle({
    source: {
      builtinDir: BUILTIN_DIR,
      remoteDir: Deno.makeTempDirSync({ prefix: 'wvb-bindings-' }),
    },
    routes: { '/': { bundle: 'app' } },
  });
}

Deno.test('dispatch serves source.* commands against the builtin bundle', async () => {
  const app = makeApp();

  const list = await dispatch(app, 'sourceListBundles');
  assert(list.ok);
  assert(
    (list.value as Array<{ name: string; version: string }>).some(
      b => b.name === 'app' && b.version === '1.0.0'
    )
  );

  assertEquals(await dispatch(app, 'sourceLoadVersion', { bundleName: 'app' }), {
    ok: true,
    value: { type: 'builtin', version: '1.0.0' },
  });
});

Deno.test('dispatch returns handler_not_found for an unknown command', async () => {
  const res = await dispatch(makeApp(), 'nope');
  assertEquals(res, {
    ok: false,
    error: { code: 'handler_not_found', message: 'no invoke handler registered for "nope"' },
  });
});

Deno.test('dispatch fails closed when remote/updater are not configured', async () => {
  const app = makeApp();
  const remote = await dispatch(app, 'remoteListBundles', {});
  assert(!remote.ok);
  assertEquals(remote.error.code, 'remote_not_initialized');

  const updater = await dispatch(app, 'updaterListRemotes');
  assert(!updater.ok);
  assertEquals(updater.error.code, 'updater_not_initialized');
});

Deno.test('registerBindings binds wvbInvoke as the transport', async () => {
  const app = makeApp();
  const bound = new Map<string, (...args: any[]) => unknown>();
  const win: DenoBrowserWindow = { bind: (name, handler) => bound.set(name, handler) };

  registerBindings(win, app);
  assert(bound.has(INVOKE_BINDING));

  const result = await bound.get(INVOKE_BINDING)!('sourceLoadVersion', { bundleName: 'app' });
  assertEquals(result, { ok: true, value: { type: 'builtin', version: '1.0.0' } });
});
