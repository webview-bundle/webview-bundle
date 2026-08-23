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
  sourceLoadVersion: async (wvb, { bundleName }) => wvb.source.loadVersion(bundleName),
  sourceUpdateVersion: async (wvb, { bundleName, version }) =>
    wvb.source.updateRemoteVersion(bundleName, version),
  sourceResolveFilepath: async (wvb, { bundleName }) => wvb.source.resolveFilepath(bundleName),
  sourceGetBuiltinBundleFilepath: async (wvb, { bundleName, version }) =>
    wvb.source.getBuiltinBundleFilepath(bundleName, version),
  sourceGetRemoteBundleFilepath: async (wvb, { bundleName, version }) =>
    wvb.source.getRemoteBundleFilepath(bundleName, version),
  sourceLoadBuiltinMetadata: async (wvb, { bundleName, version }) =>
    wvb.source.loadBuiltinMetadata(bundleName, version),
  sourceLoadRemoteMetadata: async (wvb, { bundleName, version }) =>
    wvb.source.loadRemoteMetadata(bundleName, version),
  sourceUnloadDescriptor: async (wvb, { bundleName }) => wvb.source.unloadDescriptor(bundleName),
  sourceRemoveRemoteBundle: async (wvb, { bundleName, version }) =>
    wvb.source.removeRemoteBundle(bundleName, version),
  sourceRemoteRetainedVersions: async (wvb, { bundleName }) =>
    wvb.source.remoteRetainedVersions(bundleName),
  sourcePruneRemoteBundles: async (wvb, { bundleName }) =>
    wvb.source.pruneRemoteBundles(bundleName),
  // remote
  remoteListBundles: async (wvb, { channel }) => ensureRemote(wvb).listBundles({ channel }),
  remoteGetInfo: async (wvb, { bundleName, channel }) =>
    ensureRemote(wvb).getInfo(bundleName, { channel }),
  remoteDownload: async (wvb, { bundleName, channel }) => {
    const [info] = await ensureRemote(wvb).download(bundleName, channel);
    return info;
  },
  remoteDownloadVersion: async (wvb, { bundleName, version }) => {
    const [info] = await ensureRemote(wvb).downloadVersion(bundleName, version);
    return info;
  },
  // updater
  updaterListRemotes: async wvb => ensureUpdater(wvb).listRemotes(),
  updaterGetUpdate: async (wvb, { bundleName }) => ensureUpdater(wvb).getUpdate(bundleName),
  updaterDownload: async (wvb, { bundleName, version }) =>
    ensureUpdater(wvb).download(bundleName, version),
  updaterInstall: async (wvb, { bundleName, version }) => {
    await ensureUpdater(wvb).install(bundleName, version);
  },
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
