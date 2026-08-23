import { invoke } from './invoke.js';

/**
 * - builtin: Built-in bundle which is included in the application.
 * - remote: Remote bundle which is downloaded from a remote server.
 */
export type SourceKind = 'builtin' | 'remote';

/** What the manifest records for one version of a bundle. */
export interface ManifestVersionData {
  /** Integrity value to verify the bundle. */
  integrity?: string;
  /** Arbitrary string-valued metadata the update carried for this version. */
  metadata?: Record<string, string>;
}

/** Where a version stands in its bundle's lifecycle. */
export type ManifestBundleItemStatus = 'current' | 'previous' | 'staged' | 'orphan';

/** One version of one bundle, as the manifest records it. */
export interface ManifestBundleItem {
  /** The name of the bundle. */
  name: string;
  /** The version of the bundle. */
  version: string;
  /** Whether this version is the current/previous/staged one, or an orphan. */
  status: ManifestBundleItemStatus;
  /** Bundle manifest data. */
  data: ManifestVersionData;
}

/** List item of bundles. */
export interface SourceListItem {
  /** Which source (builtin or remote) records this item. */
  source: SourceKind;
  /** The bundle item. */
  item: ManifestBundleItem;
}

/** Bundle version with the source that provides it. */
export interface BundleSourceVersion {
  /** The source that provides this version. */
  source: SourceKind;
  /** The version of the bundle. */
  version: string;
}

export type ManifestSetCurrentVersionResultKind =
  /** The version is now the current one. */
  | 'settled'
  /** The bundle does not exist in the manifest. */
  | 'not_exists'
  /** The version does not exist in the manifest. */
  | 'version_not_exists';

export interface ManifestSetCurrentVersionResult {
  name: string;
  version: string;
  kind: ManifestSetCurrentVersionResultKind;
}

/** The version to stage, and what the manifest should record for it. */
export interface ManifestStageData {
  version: string;
  data?: ManifestVersionData;
}

export type ManifestStageResultKind =
  /** The version is now the staged one. */
  | 'staged'
  /** The version is the current one, so it cannot be staged. */
  | 'in_use';

export interface ManifestStageResult {
  name: string;
  version: string;
  kind: ManifestStageResultKind;
}

/** The versions to remove from one bundle. */
export interface ManifestRemoveData {
  versions: string[];
  /** Remove even the version in use. Default: `false`. */
  force?: boolean;
}

export type ManifestRemoveResultKind =
  | 'removed'
  /** The bundle does not exist in the manifest. */
  | 'not_exists'
  /** The version does not exist in the manifest. */
  | 'version_not_exists'
  /** The version is the current one; pass `force` to remove it anyway. */
  | 'in_use';

export interface ManifestRemoveResult {
  name: string;
  version: string;
  kind: ManifestRemoveResultKind;
}

export interface ManifestPruneResult {
  name: string;
  /** The orphan versions that were removed. */
  prunedVersions: string[];
}

async function listBundles(): Promise<SourceListItem[]> {
  return invoke<SourceListItem[]>('sourceListBundles');
}

async function listBuiltinBundles(): Promise<SourceListItem[]> {
  return invoke<SourceListItem[]>('sourceListBuiltinBundles');
}

async function listRemoteBundles(): Promise<SourceListItem[]> {
  return invoke<SourceListItem[]>('sourceListRemoteBundles');
}

async function getVersion(bundleName: string): Promise<BundleSourceVersion | null> {
  return invoke<BundleSourceVersion | null>('sourceGetVersion', { bundleName });
}

async function getRemoteStagedVersion(bundleName: string): Promise<string | null> {
  return invoke<string | null>('sourceGetRemoteStagedVersion', { bundleName });
}

async function getRemotePreviousVersion(bundleName: string): Promise<string | null> {
  return invoke<string | null>('sourceGetRemotePreviousVersion', { bundleName });
}

async function getBuiltinVersionData(
  bundleName: string,
  version: string
): Promise<ManifestVersionData | null> {
  return invoke<ManifestVersionData | null>('sourceGetBuiltinVersionData', {
    bundleName,
    version,
  });
}

async function getRemoteVersionData(
  bundleName: string,
  version: string
): Promise<ManifestVersionData | null> {
  return invoke<ManifestVersionData | null>('sourceGetRemoteVersionData', {
    bundleName,
    version,
  });
}

async function updateRemoteVersion(
  bundleName: string,
  version: string
): Promise<ManifestSetCurrentVersionResult> {
  return invoke<ManifestSetCurrentVersionResult>('sourceUpdateRemoteVersion', {
    bundleName,
    version,
  });
}

async function updateRemoteVersions(
  items: Record<string, string>
): Promise<ManifestSetCurrentVersionResult[]> {
  return invoke<ManifestSetCurrentVersionResult[]>('sourceUpdateRemoteVersions', { items });
}

async function stageRemoteBundle(
  bundleName: string,
  data: ManifestStageData
): Promise<ManifestStageResult> {
  return invoke<ManifestStageResult>('sourceStageRemoteBundle', { bundleName, data });
}

async function stageRemoteBundles(
  items: Record<string, ManifestStageData>
): Promise<ManifestStageResult[]> {
  return invoke<ManifestStageResult[]>('sourceStageRemoteBundles', { items });
}

async function removeRemoteBundle(
  bundleName: string,
  version: string,
  force?: boolean
): Promise<ManifestRemoveResult> {
  return invoke<ManifestRemoveResult>('sourceRemoveRemoteBundle', { bundleName, version, force });
}

async function removeRemoteBundles(
  items: Record<string, ManifestRemoveData>
): Promise<ManifestRemoveResult[]> {
  return invoke<ManifestRemoveResult[]>('sourceRemoveRemoteBundles', { items });
}

async function pruneRemoteBundle(bundleName: string): Promise<ManifestPruneResult> {
  return invoke<ManifestPruneResult>('sourcePruneRemoteBundle', { bundleName });
}

async function pruneRemoteBundles(bundleNames: string[]): Promise<ManifestPruneResult[]> {
  return invoke<ManifestPruneResult[]>('sourcePruneRemoteBundles', { bundleNames });
}

async function resolveFilepath(bundleName: string): Promise<string> {
  return invoke<string>('sourceResolveFilepath', { bundleName });
}

async function getBuiltinBundleFilepath(bundleName: string, version: string): Promise<string> {
  return invoke<string>('sourceGetBuiltinBundleFilepath', { bundleName, version });
}

async function getRemoteBundleFilepath(bundleName: string, version: string): Promise<string> {
  return invoke<string>('sourceGetRemoteBundleFilepath', { bundleName, version });
}

async function unload(bundleName: string): Promise<boolean> {
  return invoke<boolean>('sourceUnload', { bundleName });
}

export interface SourceApi {
  listBundles: typeof listBundles;
  listBuiltinBundles: typeof listBuiltinBundles;
  listRemoteBundles: typeof listRemoteBundles;
  getVersion: typeof getVersion;
  getRemoteStagedVersion: typeof getRemoteStagedVersion;
  getRemotePreviousVersion: typeof getRemotePreviousVersion;
  getBuiltinVersionData: typeof getBuiltinVersionData;
  getRemoteVersionData: typeof getRemoteVersionData;
  updateRemoteVersion: typeof updateRemoteVersion;
  updateRemoteVersions: typeof updateRemoteVersions;
  stageRemoteBundle: typeof stageRemoteBundle;
  stageRemoteBundles: typeof stageRemoteBundles;
  removeRemoteBundle: typeof removeRemoteBundle;
  removeRemoteBundles: typeof removeRemoteBundles;
  pruneRemoteBundle: typeof pruneRemoteBundle;
  pruneRemoteBundles: typeof pruneRemoteBundles;
  resolveFilepath: typeof resolveFilepath;
  getBuiltinBundleFilepath: typeof getBuiltinBundleFilepath;
  getRemoteBundleFilepath: typeof getRemoteBundleFilepath;
  unload: typeof unload;
}

export const source: SourceApi = {
  listBundles,
  listBuiltinBundles,
  listRemoteBundles,
  getVersion,
  getRemoteStagedVersion,
  getRemotePreviousVersion,
  getBuiltinVersionData,
  getRemoteVersionData,
  updateRemoteVersion,
  updateRemoteVersions,
  stageRemoteBundle,
  stageRemoteBundles,
  removeRemoteBundle,
  removeRemoteBundles,
  pruneRemoteBundle,
  pruneRemoteBundles,
  resolveFilepath,
  getBuiltinBundleFilepath,
  getRemoteBundleFilepath,
  unload,
};
