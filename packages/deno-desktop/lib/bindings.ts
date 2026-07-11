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
  remoteListBundles: (wvb, { channel }) => ensureRemote(wvb).listBundles(channel),
  remoteGetInfo: (wvb, { bundleName, channel }) => ensureRemote(wvb).getInfo(bundleName, channel),
  remoteDownload: async (wvb, { bundleName, channel }) =>
    (await ensureRemote(wvb).download(bundleName, channel)).info,
  remoteDownloadVersion: async (wvb, { bundleName, version }) =>
    (await ensureRemote(wvb).downloadVersion(bundleName, version)).info,
  // updater
  updaterListRemotes: wvb => ensureUpdater(wvb).listRemotes(),
  updaterGetUpdate: (wvb, { bundleName }) => ensureUpdater(wvb).getUpdate(bundleName),
  updaterDownload: (wvb, { bundleName, version }) =>
    ensureUpdater(wvb).download(bundleName, version),
  updaterInstall: async (wvb, { bundleName, version }) => {
    await ensureUpdater(wvb).install(bundleName, version);
  },
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
