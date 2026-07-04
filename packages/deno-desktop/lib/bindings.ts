// Host-side @wvb/bridge transport for Deno desktop. Registers a single `wvbInvoke` binding on a
// `Deno.BrowserWindow` (mirrors @wvb/electron's `registerIpc` + `window.wvbElectron.invoke`) that
// dispatches @wvb/bridge `source.*` / `remote.*` / `updater.*` commands to a WebviewBundle.
// See https://docs.deno.com/runtime/desktop/bindings/.
import type { Remote, Updater } from '@wvb/deno';
import type { WebviewBundle } from './webview-bundle.ts';

/** The single binding name the `@wvb/bridge` `deno` transport calls. */
export const INVOKE_BINDING = 'wvbInvoke';

/** Error payload returned to `@wvb/bridge` (becomes a `BridgeError` there, preserving `code`). */
export interface BridgeErrorData {
  code?: string;
  message: string;
}

/**
 * Result envelope. Deno desktop delivers a thrown handler error as `{ name, message, stack }`
 * (dropping our `code`), so handlers never throw across the binding — they return this instead and
 * `@wvb/bridge` unwraps it (mirrors @wvb/electron's preload).
 */
export type InvokeResult = { ok: true; value: unknown } | { ok: false; error: BridgeErrorData };

export const BridgeErrorCode = {
  RemoteNotInitialized: 'remote_not_initialized',
  UpdaterNotInitialized: 'updater_not_initialized',
  HandlerNotFound: 'handler_not_found',
} as const;

class BridgeError extends Error {
  override readonly name = 'BridgeError';

  constructor(
    readonly code: string,
    message: string
  ) {
    super(message);
  }
}

function requireRemote(wvb: WebviewBundle): Remote {
  if (wvb.remote == null) {
    throw new BridgeError(BridgeErrorCode.RemoteNotInitialized, 'remote is not initialized.');
  }
  return wvb.remote;
}

function requireUpdater(wvb: WebviewBundle): Updater {
  if (wvb.updater == null) {
    throw new BridgeError(BridgeErrorCode.UpdaterNotInitialized, 'updater is not initialized.');
  }
  return wvb.updater;
}

type Params = Record<string, any>;
type Handler = (wvb: WebviewBundle, params: Params) => Promise<unknown> | unknown;

const handlers: Record<string, Handler> = {
  // source
  sourceListBundles: wvb => wvb.source.listBundles(),
  sourceLoadVersion: (wvb, { bundleName }) => wvb.source.loadVersion(bundleName),
  sourceUpdateVersion: (wvb, { bundleName, version }) =>
    wvb.source.updateRemoteVersion(bundleName, version),
  sourceResolveFilepath: (wvb, { bundleName }) => wvb.source.resolveFilepath(bundleName),
  sourceGetBuiltinBundleFilepath: (wvb, { bundleName, version }) =>
    wvb.source.getBuiltinBundleFilepath(bundleName, version),
  sourceGetRemoteBundleFilepath: (wvb, { bundleName, version }) =>
    wvb.source.getRemoteBundleFilepath(bundleName, version),
  sourceLoadBuiltinMetadata: (wvb, { bundleName, version }) =>
    wvb.source.loadBuiltinMetadata(bundleName, version),
  sourceLoadRemoteMetadata: (wvb, { bundleName, version }) =>
    wvb.source.loadRemoteMetadata(bundleName, version),
  sourceUnloadDescriptor: (wvb, { bundleName }) => wvb.source.unloadDescriptor(bundleName),
  sourceRemoveRemoteBundle: (wvb, { bundleName, version }) =>
    wvb.source.removeRemoteBundle(bundleName, version),
  sourceRemoteRetainedVersions: (wvb, { bundleName }) =>
    wvb.source.remoteRetainedVersions(bundleName),
  sourcePruneRemoteBundles: (wvb, { bundleName }) => wvb.source.pruneRemoteBundles(bundleName),
  // remote
  remoteListBundles: (wvb, { channel }) => requireRemote(wvb).listBundles(channel),
  remoteGetInfo: (wvb, { bundleName, channel }) => requireRemote(wvb).getInfo(bundleName, channel),
  remoteDownload: async (wvb, { bundleName, channel }) =>
    (await requireRemote(wvb).download(bundleName, channel)).info,
  remoteDownloadVersion: async (wvb, { bundleName, version }) =>
    (await requireRemote(wvb).downloadVersion(bundleName, version)).info,
  // updater
  updaterListRemotes: wvb => requireUpdater(wvb).listRemotes(),
  updaterGetUpdate: (wvb, { bundleName }) => requireUpdater(wvb).getUpdate(bundleName),
  updaterDownload: (wvb, { bundleName, version }) =>
    requireUpdater(wvb).download(bundleName, version),
  updaterInstall: async (wvb, { bundleName, version }) => {
    await requireUpdater(wvb).install(bundleName, version);
  },
};

function toErrorData(error: unknown): BridgeErrorData {
  if (error instanceof BridgeError) {
    return { code: error.code, message: error.message };
  }
  if (error instanceof Error) {
    return { message: error.message };
  }
  return { message: typeof error === 'string' ? error : 'unknown error' };
}

/** Names of every command this host can serve. */
export const handlerNames: readonly string[] = Object.keys(handlers);

/** Run one `@wvb/bridge` command and return its JSON-serializable result envelope (never throws). */
export async function dispatch(
  wvb: WebviewBundle,
  name: string,
  params?: Params
): Promise<InvokeResult> {
  const handler = handlers[name];
  if (handler == null) {
    return {
      ok: false,
      error: {
        code: BridgeErrorCode.HandlerNotFound,
        message: `no invoke handler registered for "${name}"`,
      },
    };
  }
  try {
    // `?? null` so void handlers (update/install) return a JSON value rather than dropped `undefined`.
    return { ok: true, value: (await handler(wvb, params ?? {})) ?? null };
  } catch (error) {
    return { ok: false, error: toErrorData(error) };
  }
}

/** A `Deno.BrowserWindow` (only the binding methods we use; the full type ships with Deno desktop). */
export interface DenoBrowserWindow {
  bind(name: string, handler: (...args: any[]) => unknown): void;
  unbind?(name: string): void;
}

/**
 * Register the `@wvb/bridge` transport on a Deno desktop window: a single `wvbInvoke(name, params)`
 * binding that dispatches to `wvb`. Call after creating the window and the app, e.g.
 *
 * ```ts
 * const win = new Deno.BrowserWindow();
 * const app = webviewBundle({ source: { appName: 'myapp' }, protocols: [bundleProtocol('app')] });
 * registerBindings(win, app);
 * Deno.serve(app.fetch);
 * ```
 */
export function registerBindings(win: DenoBrowserWindow, wvb: WebviewBundle): void {
  win.bind(INVOKE_BINDING, (name: string, params?: Params) => dispatch(wvb, name, params));
}
