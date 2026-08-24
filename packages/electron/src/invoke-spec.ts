import type {
  BundleSourceVersion,
  BundleUpdate,
  ManifestPruneResult,
  ManifestRemoveData,
  ManifestRemoveResult,
  ManifestSetCurrentVersionResult,
  ManifestStageData,
  ManifestStageResult,
  ManifestVersionData,
  RemoteGetUpdateOptions,
  RemoteUpdateResponse,
  SourceListItem,
  Update,
  UpdaterDownloadOptions,
  UpdaterDownloadResult,
  UpdaterGetUpdateOptions,
  UpdaterInstallResult,
  UpdaterInstallTarget,
  UpdaterRollbackResult,
  UpdaterRollbackTarget,
} from '@wvb/node';

/** Single IPC channel backing the `@wvb/bridge` Electron transport. */
export const INVOKE_CHANNEL = 'webview-bundle:invoke';

export interface BridgeErrorData {
  code?: string;
  message: string;
}

export type InvokeResult = { ok: true; value: unknown } | { ok: false; error: BridgeErrorData };

/** Wire protocol for `window.wvbElectron.invoke`. */
export interface InvokeSpecs {
  sourceListBundles: { params: undefined; ok: SourceListItem[] };
  sourceListBuiltinBundles: { params: undefined; ok: SourceListItem[] };
  sourceListRemoteBundles: { params: undefined; ok: SourceListItem[] };
  sourceGetVersion: { params: { bundleName: string }; ok: BundleSourceVersion | null };
  sourceGetRemoteStagedVersion: { params: { bundleName: string }; ok: string | null };
  sourceGetRemotePreviousVersion: { params: { bundleName: string }; ok: string | null };
  sourceGetBuiltinVersionData: {
    params: { bundleName: string; version: string };
    ok: ManifestVersionData | null;
  };
  sourceGetRemoteVersionData: {
    params: { bundleName: string; version: string };
    ok: ManifestVersionData | null;
  };
  sourceUpdateRemoteVersion: {
    params: { bundleName: string; version: string };
    ok: ManifestSetCurrentVersionResult;
  };
  sourceUpdateRemoteVersions: {
    params: { items: Record<string, string> };
    ok: ManifestSetCurrentVersionResult[];
  };
  sourceStageRemoteBundle: {
    params: { bundleName: string; data: ManifestStageData };
    ok: ManifestStageResult;
  };
  sourceStageRemoteBundles: {
    params: { items: Record<string, ManifestStageData> };
    ok: ManifestStageResult[];
  };
  sourceRemoveRemoteBundle: {
    params: { bundleName: string; version: string; force?: boolean };
    ok: ManifestRemoveResult;
  };
  sourceRemoveRemoteBundles: {
    params: { items: Record<string, ManifestRemoveData> };
    ok: ManifestRemoveResult[];
  };
  sourcePruneRemoteBundle: { params: { bundleName: string }; ok: ManifestPruneResult };
  sourcePruneRemoteBundles: { params: { bundleNames: string[] }; ok: ManifestPruneResult[] };
  sourceResolveFilepath: { params: { bundleName: string }; ok: string };
  sourceGetBuiltinBundleFilepath: { params: { bundleName: string; version: string }; ok: string };
  sourceGetRemoteBundleFilepath: { params: { bundleName: string; version: string }; ok: string };
  sourceUnload: { params: { bundleName: string }; ok: boolean };
  remoteGetUpdate: {
    params: { options?: RemoteGetUpdateOptions };
    ok: RemoteUpdateResponse | null;
  };
  remoteDownload: { params: { url: string; filepath: string }; ok: void };
  updaterGetUpdate: { params: { options?: UpdaterGetUpdateOptions }; ok: Update | null };
  updaterDownload: {
    params: { bundleUpdates: BundleUpdate[]; options?: UpdaterDownloadOptions };
    ok: UpdaterDownloadResult[];
  };
  updaterInstall: { params: { targets: UpdaterInstallTarget[] }; ok: UpdaterInstallResult[] };
  updaterRollback: { params: { targets: UpdaterRollbackTarget[] }; ok: UpdaterRollbackResult[] };
}

export type InvokeName = keyof InvokeSpecs;
export type InvokeParams<K extends InvokeName> = InvokeSpecs[K]['params'];
export type InvokeOk<K extends InvokeName> = InvokeSpecs[K]['ok'];
