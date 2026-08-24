import type { Remote, Source, Updater } from '@wvb/node';

export type {
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

/** APIs exposed to a renderer through Electron IPC. */
export interface WebviewBundleSourceApi
  extends Pick<
    Source,
    | 'listBundles'
    | 'listBuiltinBundles'
    | 'listRemoteBundles'
    | 'getVersion'
    | 'getRemoteStagedVersion'
    | 'getRemotePreviousVersion'
    | 'getBuiltinVersionData'
    | 'getRemoteVersionData'
    | 'updateRemoteVersion'
    | 'updateRemoteVersions'
    | 'stageRemoteBundle'
    | 'stageRemoteBundles'
    | 'removeRemoteBundle'
    | 'removeRemoteBundles'
    | 'pruneRemoteBundle'
    | 'pruneRemoteBundles'
    | 'resolveFilepath'
    | 'getBuiltinBundleFilepath'
    | 'getRemoteBundleFilepath'
    | 'unload'
  > {}

export interface WebviewBundleRemoteApi extends Pick<Remote, 'getUpdate' | 'download'> {}

export interface WebviewBundleUpdaterApi
  extends Pick<Updater, 'getUpdate' | 'download' | 'install' | 'rollback'> {}

export interface WebviewBundleApi {
  readonly source: WebviewBundleSourceApi;
  readonly remote: WebviewBundleRemoteApi;
  readonly updater: WebviewBundleUpdaterApi;
}
