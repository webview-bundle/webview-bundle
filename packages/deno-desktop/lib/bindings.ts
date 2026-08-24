import { isWebviewBundleError, type Remote, type Updater } from '@wvb/deno';
import type { WebviewBundle } from './webview-bundle.ts';

export const INVOKE_BINDING = 'wvbInvoke';

/** Error payload returned to `@wvb/bridge`. */
export interface BridgeErrorData {
  code?: string;
  message: string;
}

/**
 * Result envelope. Deno desktop delivers a thrown handler error as `{ name, message, stack }`
 * (dropping our `code`), so handlers never throw across the binding.
 */
export type InvokeResult = { ok: true; value: unknown } | { ok: false; error: BridgeErrorData };

class BridgeError extends Error {
  override readonly name = 'BridgeError';

  constructor(
    readonly code: string,
    message: string
  ) {
    super(message);
  }
}

function ensureRemote(wvb: WebviewBundle): Remote {
  if (wvb.remote == null) {
    throw new BridgeError('remote_not_initialized', 'remote is not initialized.');
  }
  return wvb.remote;
}

function ensureUpdater(wvb: WebviewBundle): Updater {
  if (wvb.updater == null) {
    throw new BridgeError('updater_not_initialized', 'updater is not initialized.');
  }
  return wvb.updater;
}

type Params = Record<string, any>;
type Handler = (wvb: WebviewBundle, params: Params) => Promise<unknown> | unknown;

// The command names and their parameters mirror the Tauri plugin's, so `@wvb/bridge` speaks one
// vocabulary across hosts. A verifying key is never accepted from the webview: `remoteGetUpdate`
// forwards only the etag/channel, and the keys the updater checks against come from the host config.
const handlers: Record<string, Handler> = {
  // source
  sourceListBundles: wvb => wvb.source.listBundles(),
  sourceListBuiltinBundles: wvb => wvb.source.listBuiltinBundles(),
  sourceListRemoteBundles: wvb => wvb.source.listRemoteBundles(),
  sourceGetVersion: (wvb, { bundleName }) => wvb.source.getVersion(bundleName),
  sourceGetRemoteStagedVersion: (wvb, { bundleName }) =>
    wvb.source.getRemoteStagedVersion(bundleName),
  sourceGetRemotePreviousVersion: (wvb, { bundleName }) =>
    wvb.source.getRemotePreviousVersion(bundleName),
  sourceGetBuiltinVersionData: (wvb, { bundleName, version }) =>
    wvb.source.getBuiltinVersionData(bundleName, version),
  sourceGetRemoteVersionData: (wvb, { bundleName, version }) =>
    wvb.source.getRemoteVersionData(bundleName, version),
  sourceUpdateRemoteVersion: (wvb, { bundleName, version }) =>
    wvb.source.updateRemoteVersion(bundleName, version),
  sourceUpdateRemoteVersions: (wvb, { items }) => wvb.source.updateRemoteVersions(items),
  sourceStageRemoteBundle: (wvb, { bundleName, data }) =>
    wvb.source.stageRemoteBundle(bundleName, data),
  sourceStageRemoteBundles: (wvb, { items }) => wvb.source.stageRemoteBundles(items),
  sourceRemoveRemoteBundle: (wvb, { bundleName, version, force }) =>
    wvb.source.removeRemoteBundle(bundleName, version, force),
  sourceRemoveRemoteBundles: (wvb, { items }) => wvb.source.removeRemoteBundles(items),
  sourcePruneRemoteBundle: (wvb, { bundleName }) => wvb.source.pruneRemoteBundle(bundleName),
  sourcePruneRemoteBundles: (wvb, { bundleNames }) => wvb.source.pruneRemoteBundles(bundleNames),
  sourceResolveFilepath: (wvb, { bundleName }) => wvb.source.resolveFilepath(bundleName),
  sourceGetBuiltinBundleFilepath: (wvb, { bundleName, version }) =>
    wvb.source.getBuiltinBundleFilepath(bundleName, version),
  sourceGetRemoteBundleFilepath: (wvb, { bundleName, version }) =>
    wvb.source.getRemoteBundleFilepath(bundleName, version),
  sourceUnload: (wvb, { bundleName }) => wvb.source.unload(bundleName),
  // remote
  remoteGetUpdate: (wvb, { options }) =>
    ensureRemote(wvb).getUpdate({ etag: options?.etag, channel: options?.channel }),
  remoteDownload: async (wvb, { url, filepath }) => {
    await ensureRemote(wvb).download(url, filepath);
  },
  // updater
  updaterGetUpdate: (wvb, { options }) => ensureUpdater(wvb).getUpdate(options),
  updaterDownload: (wvb, { bundleUpdates, options }) =>
    ensureUpdater(wvb).download(bundleUpdates, options),
  updaterInstall: (wvb, { targets }) => ensureUpdater(wvb).install(targets),
  updaterRollback: (wvb, { targets }) => ensureUpdater(wvb).rollback(targets),
};

function toErrorData(error: unknown): BridgeErrorData {
  if (error instanceof BridgeError) {
    return { code: error.code, message: error.message };
  }
  if (isWebviewBundleError(error)) {
    return { code: error.code, message: error.message };
  }
  if (error instanceof Error) {
    return { message: error.message };
  }
  return { message: typeof error === 'string' ? error : 'unknown error' };
}

/** Names of every command this host can serve. */
export const handlerNames: readonly string[] = Object.keys(handlers);

async function dispatch(wvb: WebviewBundle, name: string, params?: Params): Promise<InvokeResult> {
  const handler = handlers[name];
  if (handler == null) {
    return {
      ok: false,
      error: {
        code: 'handler_not_found',
        message: `no invoke handler registered for "${name}"`,
      },
    };
  }
  try {
    return { ok: true, value: (await handler(wvb, params ?? {})) ?? null };
  } catch (error) {
    return { ok: false, error: toErrorData(error) };
  }
}

export interface DenoBrowserWindow {
  bind(name: string, handler: (...args: any[]) => unknown): void;
  unbind?(name: string): void;
}

/**
 * Register the `@wvb/bridge` transport on a Deno desktop window.
 *
 * ```ts
 * const win = new Deno.BrowserWindow();
 * const wvb = webviewBundle({ source: { appName: 'myapp' }, routes: { '/': { bundle: 'app' } } });
 * registerBindings(win, wvb);
 * Deno.serve(wvb.fetch);
 * ```
 */
export function registerBindings(win: DenoBrowserWindow, wvb: WebviewBundle): void {
  win.bind(INVOKE_BINDING, (name: string, params?: Params) => dispatch(wvb, name, params));
}
