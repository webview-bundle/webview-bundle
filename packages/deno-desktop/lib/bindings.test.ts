import { assert, assertEquals } from '@std/assert';
import { fromFileUrl } from '@std/path';
import { loadLib } from '@wvb/deno';
import {
  type DenoBrowserWindow,
  INVOKE_BINDING,
  type InvokeResult,
  registerBindings,
} from './bindings.ts';
import { type Routes, webviewBundle } from './mod.ts';

const ext = Deno.build.os === 'windows' ? 'dll' : Deno.build.os === 'darwin' ? 'dylib' : 'so';
const prefix = Deno.build.os === 'windows' ? '' : 'lib';
const DYLIB = fromFileUrl(
  new URL(`../../../target/release/${prefix}wvb_deno.${ext}`, import.meta.url)
);
const BUILTIN_DIR = fromFileUrl(new URL('../fixtures/builtin', import.meta.url));

loadLib(DYLIB);

type Invoke = (name: string, params?: Record<string, unknown>) => Promise<InvokeResult>;

/** The bound `wvbInvoke` transport, over an app with no remote/updater configured. */
function invoker(routes: Routes = { '/': { bundle: 'app' } }): Invoke {
  const app = webviewBundle({
    source: { builtinDir: BUILTIN_DIR, remoteDir: Deno.makeTempDirSync({ prefix: 'wvb-test-' }) },
    routes,
  });
  let bound: ((...args: any[]) => unknown) | undefined;
  const win: DenoBrowserWindow = {
    bind: (name, handler) => {
      assertEquals(name, INVOKE_BINDING);
      bound = handler;
    },
  };
  registerBindings(win, app);
  assert(bound != null, 'registerBindings bound the transport');
  const transport = bound;
  return (name, params) => Promise.resolve(transport(name, params) as Promise<InvokeResult>);
}

Deno.test('the bridge serves the source commands', async () => {
  const invoke = invoker();

  const listed = await invoke('sourceListBundles');
  assertEquals(listed, {
    ok: true,
    value: [
      {
        source: 'builtin',
        item: { name: 'app', version: '1.0.0', status: 'current', data: {} },
      },
    ],
  });

  assertEquals(await invoke('sourceGetVersion', { bundleName: 'app' }), {
    ok: true,
    value: { source: 'builtin', version: '1.0.0' },
  });
  assertEquals(await invoke('sourceListRemoteBundles'), { ok: true, value: [] });
  assertEquals(await invoke('sourceUnload', { bundleName: 'app' }), { ok: true, value: false });
  assertEquals(await invoke('sourceGetRemoteStagedVersion', { bundleName: 'app' }), {
    ok: true,
    value: null,
  });
});

Deno.test('the bridge reports a failed command instead of throwing', async () => {
  const invoke = invoker();

  const missing = await invoke('nope');
  assertEquals(missing, {
    ok: false,
    error: { code: 'handler_not_found', message: 'no invoke handler registered for "nope"' },
  });

  // An error from the binding keeps its stable code, so the webview can branch on it.
  const unknownBundle = (await invoke('sourceResolveFilepath', {
    bundleName: 'nope',
  })) as Extract<InvokeResult, { ok: false }>;
  assertEquals(unknownBundle.ok, false);
  assertEquals(unknownBundle.error.code, 'core.bundle_not_found');
});

Deno.test('the remote and updater commands report that they are not configured', async () => {
  const invoke = invoker();

  assertEquals(await invoke('remoteGetUpdate', {}), {
    ok: false,
    error: { code: 'remote_not_initialized', message: 'remote is not initialized.' },
  });
  assertEquals(await invoke('updaterGetUpdate', {}), {
    ok: false,
    error: { code: 'updater_not_initialized', message: 'updater is not initialized.' },
  });
});
