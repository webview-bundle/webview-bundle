import type { Remote, Updater } from '@wvb/node';
import { isWebviewBundleError } from '@wvb/node/binding';
import { ipcMain } from 'electron';
import {
  type BridgeErrorData,
  INVOKE_CHANNEL,
  type InvokeName,
  type InvokeOk,
  type InvokeParams,
  type InvokeResult,
} from './invoke-spec.js';
import type { WebviewBundle } from './webview-bundle.js';

export type BridgeErrorCode =
  | 'remote_not_initialized'
  | 'updater_not_initialized'
  | 'handler_not_found';

class BridgeError extends Error {
  override readonly name = 'BridgeError';

  constructor(
    readonly code: BridgeErrorCode,
    message: string
  ) {
    super(message);
  }
}

type InvokeHandler<K extends InvokeName> = (
  wvb: WebviewBundle,
  params: InvokeParams<K>
) => Promise<InvokeOk<K>>;

type InvokeHandlers = {
  [K in InvokeName]: InvokeHandler<K>;
};

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

function toBridgeErrorData(error: unknown): BridgeErrorData {
  if (error instanceof BridgeError) {
    return { code: error.code, message: error.message };
  }
  // `@wvb/node` errors already carry a stable code (`core.*` for core failures; binding-local ones
  // like `napi`/`null_handle` are unprefixed) — forward it so the renderer's `BridgeError`
  // identifies the failure the same way the native side did.
  if (isWebviewBundleError(error)) {
    return { code: error.code, message: error.message };
  }
  if (error instanceof Error) {
    return { message: error.message };
  }
  return { message: typeof error === 'string' ? error : 'unknown error' };
}

const handlers: InvokeHandlers = {
  // source
  sourceListBundles: async wvb => wvb.source.listBundles(),
  sourceListBuiltinBundles: async wvb => wvb.source.listBuiltinBundles(),
  sourceListRemoteBundles: async wvb => wvb.source.listRemoteBundles(),
  sourceGetVersion: async (wvb, { bundleName }) => wvb.source.getVersion(bundleName),
  sourceGetRemoteStagedVersion: async (wvb, { bundleName }) =>
    wvb.source.getRemoteStagedVersion(bundleName),
  sourceGetRemotePreviousVersion: async (wvb, { bundleName }) =>
    wvb.source.getRemotePreviousVersion(bundleName),
  sourceGetBuiltinVersionData: async (wvb, { bundleName, version }) =>
    wvb.source.getBuiltinVersionData(bundleName, version),
  sourceGetRemoteVersionData: async (wvb, { bundleName, version }) =>
    wvb.source.getRemoteVersionData(bundleName, version),
  sourceUpdateRemoteVersion: async (wvb, { bundleName, version }) =>
    wvb.source.updateRemoteVersion(bundleName, version),
  sourceUpdateRemoteVersions: async (wvb, { items }) => wvb.source.updateRemoteVersions(items),
  sourceStageRemoteBundle: async (wvb, { bundleName, data }) =>
    wvb.source.stageRemoteBundle(bundleName, data),
  sourceStageRemoteBundles: async (wvb, { items }) => wvb.source.stageRemoteBundles(items),
  sourceResolveFilepath: async (wvb, { bundleName }) => wvb.source.resolveFilepath(bundleName),
  sourceGetBuiltinBundleFilepath: async (wvb, { bundleName, version }) =>
    wvb.source.getBuiltinBundleFilepath(bundleName, version),
  sourceGetRemoteBundleFilepath: async (wvb, { bundleName, version }) =>
    wvb.source.getRemoteBundleFilepath(bundleName, version),
  sourceUnload: async (wvb, { bundleName }) => wvb.source.unload(bundleName),
  sourceRemoveRemoteBundle: async (wvb, { bundleName, version, force }) =>
    wvb.source.removeRemoteBundle(bundleName, version, force),
  sourceRemoveRemoteBundles: async (wvb, { items }) => wvb.source.removeRemoteBundles(items),
  sourcePruneRemoteBundle: async (wvb, { bundleName }) => wvb.source.pruneRemoteBundle(bundleName),
  sourcePruneRemoteBundles: async (wvb, { bundleNames }) =>
    wvb.source.pruneRemoteBundles(bundleNames),
  // remote
  remoteGetUpdate: async (wvb, { options }) => ensureRemote(wvb).getUpdate(options),
  remoteDownload: async (wvb, { url, filepath }) => ensureRemote(wvb).download(url, filepath),
  // updater
  updaterGetUpdate: async (wvb, { options }) => ensureUpdater(wvb).getUpdate(options),
  updaterDownload: async (wvb, { bundleUpdates, options }) =>
    ensureUpdater(wvb).download(bundleUpdates, options),
  updaterInstall: async (wvb, { targets }) => ensureUpdater(wvb).install(targets),
  updaterRollback: async (wvb, { targets }) => ensureUpdater(wvb).rollback(targets),
};

const handlerNames = new Set<string>(Object.keys(handlers));

export function registerIpc(wvb: WebviewBundle): void {
  ipcMain.handle(
    INVOKE_CHANNEL,
    async (_event, name: string, params: unknown): Promise<InvokeResult> => {
      const handler = (handlerNames.has(name) ? handlers[name as InvokeName] : undefined) as
        | ((wvb: WebviewBundle, params: unknown) => Promise<unknown>)
        | undefined;
      if (handler == null) {
        return {
          ok: false,
          error: toBridgeErrorData(
            new BridgeError('handler_not_found', `no invoke handler registered for "${name}"`)
          ),
        };
      }
      try {
        const value = await handler(wvb, params ?? {});
        return { ok: true, value };
      } catch (error) {
        return { ok: false, error: toBridgeErrorData(error) };
      }
    }
  );
}
