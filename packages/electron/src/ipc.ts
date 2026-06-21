import type { Remote, Updater } from '@wvb/node';
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

const ErrorCode = {
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

type InvokeHandler<K extends InvokeName> = (
  wvb: WebviewBundle,
  params: InvokeParams<K>
) => Promise<InvokeOk<K>>;

type InvokeHandlers = {
  [K in InvokeName]: InvokeHandler<K>;
};

function requireRemote(wvb: WebviewBundle): Remote {
  if (wvb.remote == null) {
    throw new BridgeError(ErrorCode.RemoteNotInitialized, 'remote is not initialized.');
  }
  return wvb.remote;
}

function requireUpdater(wvb: WebviewBundle): Updater {
  if (wvb.updater == null) {
    throw new BridgeError(ErrorCode.UpdaterNotInitialized, 'updater is not initialized.');
  }
  return wvb.updater;
}

function toBridgeErrorData(error: unknown): BridgeErrorData {
  if (error instanceof BridgeError) {
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
  remoteListBundles: async (wvb, { channel }) => requireRemote(wvb).listBundles(channel),
  remoteGetInfo: async (wvb, { bundleName, channel }) =>
    requireRemote(wvb).getInfo(bundleName, channel),
  remoteDownload: async (wvb, { bundleName, channel }) => {
    const [info] = await requireRemote(wvb).download(bundleName, channel);
    return info;
  },
  remoteDownloadVersion: async (wvb, { bundleName, version }) => {
    const [info] = await requireRemote(wvb).downloadVersion(bundleName, version);
    return info;
  },
  // updater
  updaterListRemotes: async wvb => requireUpdater(wvb).listRemotes(),
  updaterGetUpdate: async (wvb, { bundleName }) => requireUpdater(wvb).getUpdate(bundleName),
  updaterDownload: async (wvb, { bundleName, version }) =>
    requireUpdater(wvb).download(bundleName, version),
  updaterInstall: async (wvb, { bundleName, version }) => {
    await requireUpdater(wvb).install(bundleName, version);
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
          error: {
            code: ErrorCode.HandlerNotFound,
            message: `no invoke handler registered for "${name}"`,
          },
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
