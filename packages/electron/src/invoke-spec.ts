import type {
  BundleManifestMetadata,
  BundleSourceVersion,
  BundleUpdateInfo,
  ListBundleItem,
  ListRemoteBundleInfo,
  RemoteBundleInfo,
} from '@wvb/node';

/**
 * Single IPC channel backing the `@wvb/bridge` electron transport.
 */
export const INVOKE_CHANNEL = 'webview-bundle:invoke';

export interface BridgeErrorData {
  code?: string;
  message: string;
}

export type InvokeResult = { ok: true; value: unknown } | { ok: false; error: BridgeErrorData };

/**
 * Wire protocol for `window.wvbElectron.invoke`.
 */
export interface InvokeSpecs {
  // source
  sourceListBundles: { params: undefined; ok: ListBundleItem[] };
  sourceLoadVersion: {
    params: { bundleName: string };
    ok: BundleSourceVersion | null;
  };
  sourceUpdateVersion: {
    params: { bundleName: string; version: string };
    ok: void;
  };
  sourceResolveFilepath: { params: { bundleName: string }; ok: string };
  sourceGetBuiltinBundleFilepath: {
    params: { bundleName: string; version: string };
    ok: string;
  };
  sourceGetRemoteBundleFilepath: {
    params: { bundleName: string; version: string };
    ok: string;
  };
  sourceLoadBuiltinMetadata: {
    params: { bundleName: string; version: string };
    ok: BundleManifestMetadata | null;
  };
  sourceLoadRemoteMetadata: {
    params: { bundleName: string; version: string };
    ok: BundleManifestMetadata | null;
  };
  sourceUnloadDescriptor: { params: { bundleName: string }; ok: boolean };
  sourceRemoveRemoteBundle: {
    params: { bundleName: string; version: string };
    ok: boolean;
  };
  sourceRemoteRetainedVersions: { params: { bundleName: string }; ok: string[] };
  sourcePruneRemoteBundles: { params: { bundleName: string }; ok: string[] };
  // remote
  remoteListBundles: {
    params: { channel?: string | undefined };
    ok: ListRemoteBundleInfo[];
  };
  remoteGetInfo: {
    params: { bundleName: string; channel?: string | undefined };
    ok: RemoteBundleInfo;
  };
  remoteDownload: {
    params: { bundleName: string; channel?: string | undefined };
    ok: RemoteBundleInfo;
  };
  remoteDownloadVersion: {
    params: { bundleName: string; version: string };
    ok: RemoteBundleInfo;
  };
  // updater
  updaterListRemotes: { params: undefined; ok: ListRemoteBundleInfo[] };
  updaterGetUpdate: { params: { bundleName: string }; ok: BundleUpdateInfo };
  updaterDownload: {
    params: { bundleName: string; version?: string | undefined };
    ok: RemoteBundleInfo;
  };
  updaterInstall: {
    params: { bundleName: string; version: string };
    ok: void;
  };
}

export type InvokeName = keyof InvokeSpecs;
export type InvokeParams<K extends InvokeName> = InvokeSpecs[K]['params'];
export type InvokeOk<K extends InvokeName> = InvokeSpecs[K]['ok'];
