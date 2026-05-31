import type {
  BundleManifestMetadata,
  BundleSourceVersion,
  BundleUpdateInfo,
  ListBundleItem,
  ListRemoteBundleInfo,
  RemoteBundleInfo,
} from '@wvb/node';

export type {
  BundleManifestMetadata,
  BundleSourceVersion,
  BundleUpdateInfo,
  ListBundleItem,
  ListRemoteBundleInfo,
  RemoteBundleInfo,
} from '@wvb/node';

export interface WebviewBundleSourceApi {
  listBundles(): Promise<ListBundleItem[]>;
  loadVersion(bundleName: string): Promise<BundleSourceVersion | null>;
  updateVersion(bundleName: string, version: string): Promise<void>;
  filepath(bundleName: string): Promise<string>;
  getBuiltinBundleFilepath(bundleName: string, version: string): Promise<string>;
  getRemoteBundleFilepath(bundleName: string, version: string): Promise<string>;
  loadBuiltinMetadata(bundleName: string, version: string): Promise<BundleManifestMetadata | null>;
  loadRemoteMetadata(bundleName: string, version: string): Promise<BundleManifestMetadata | null>;
  unloadDescriptor(bundleName: string): Promise<boolean>;
  removeRemoteBundle(bundleName: string, version: string): Promise<boolean>;
  remoteRetainedVersions(bundleName: string): Promise<string[]>;
  pruneRemoteBundles(bundleName: string): Promise<string[]>;
}

export interface WebviewBundleRemoteApi {
  listBundles(channel?: string): Promise<ListRemoteBundleInfo[]>;
  getInfo(bundleName: string, channel?: string): Promise<RemoteBundleInfo>;
  download(bundleName: string, channel?: string): Promise<RemoteBundleInfo>;
  downloadVersion(bundleName: string, version: string): Promise<RemoteBundleInfo>;
}

export interface WebviewBundleUpdaterApi {
  listRemotes(): Promise<ListRemoteBundleInfo[]>;
  getUpdate(bundleName: string): Promise<BundleUpdateInfo>;
  download(bundleName: string, version?: string): Promise<RemoteBundleInfo>;
  install(bundleName: string, version: string): Promise<void>;
}

export interface WebviewBundleApi {
  readonly source: WebviewBundleSourceApi;
  readonly remote: WebviewBundleRemoteApi;
  readonly updater: WebviewBundleUpdaterApi;
}
