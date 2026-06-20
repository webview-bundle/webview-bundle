import { invoke } from './invoke.js';

/**
 * - builtin: Built-in bundle which is included in the application.
 * - remote: Remote bundle which is downloaded from a remote server.
 */
export type BundleSourceType = 'builtin' | 'remote';

/**
 * Bundle manifest metadata.
 */
export interface BundleManifestMetadata {
  /** "ETag" header value. */
  etag?: string;
  /** Integrity value to verify the bundle. */
  integrity?: string;
  /** Signature value to verify the bundle. */
  signature?: string;
  /** Last modified timestamp. (uses GMT) */
  lastModified?: string;
}

/**
 * List item of bundle manifests.
 */
export interface ListBundleManifestItem {
  /** The name of the bundle. */
  name: string;
  /** The version of the bundle. */
  version: string;
  /** Whether the bundle is currently active. */
  current: boolean;
  /** Bundle manifest metadata. */
  metadata: BundleManifestMetadata;
}

/**
 * List item of bundles.
 */
export interface ListBundleItem {
  /** The type of the bundle. */
  type: BundleSourceType;
  /** The bundle item. */
  item: ListBundleManifestItem;
}

/**
 * Bundle source version.
 */
export interface BundleSourceVersion {
  /** The type of the bundle source. */
  type: BundleSourceType;
  /** The version of the bundle. */
  version: string;
}

async function listBundles(): Promise<ListBundleItem[]> {
  return invoke<ListBundleItem[]>('sourceListBundles');
}

async function loadVersion(bundleName: string): Promise<BundleSourceVersion | null> {
  return invoke<BundleSourceVersion | null>('sourceLoadVersion', { bundleName });
}

async function updateVersion(bundleName: string, version: string): Promise<void> {
  return invoke<void>('sourceUpdateVersion', { bundleName, version });
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

async function loadBuiltinMetadata(
  bundleName: string,
  version: string
): Promise<BundleManifestMetadata | null> {
  return invoke<BundleManifestMetadata | null>('sourceLoadBuiltinMetadata', {
    bundleName,
    version,
  });
}

async function loadRemoteMetadata(
  bundleName: string,
  version: string
): Promise<BundleManifestMetadata | null> {
  return invoke<BundleManifestMetadata | null>('sourceLoadRemoteMetadata', {
    bundleName,
    version,
  });
}

async function unloadDescriptor(bundleName: string): Promise<boolean> {
  return invoke<boolean>('sourceUnloadDescriptor', { bundleName });
}

async function removeRemoteBundle(bundleName: string, version: string): Promise<boolean> {
  return invoke<boolean>('sourceRemoveRemoteBundle', { bundleName, version });
}

async function remoteRetainedVersions(bundleName: string): Promise<string[]> {
  return invoke<string[]>('sourceRemoteRetainedVersions', { bundleName });
}

async function pruneRemoteBundles(bundleName: string): Promise<string[]> {
  return invoke<string[]>('sourcePruneRemoteBundles', { bundleName });
}

export interface SourceApi {
  listBundles: typeof listBundles;
  loadVersion: typeof loadVersion;
  updateVersion: typeof updateVersion;
  resolveFilepath: typeof resolveFilepath;
  getBuiltinBundleFilepath: typeof getBuiltinBundleFilepath;
  getRemoteBundleFilepath: typeof getRemoteBundleFilepath;
  loadBuiltinMetadata: typeof loadBuiltinMetadata;
  loadRemoteMetadata: typeof loadRemoteMetadata;
  unloadDescriptor: typeof unloadDescriptor;
  removeRemoteBundle: typeof removeRemoteBundle;
  remoteRetainedVersions: typeof remoteRetainedVersions;
  pruneRemoteBundles: typeof pruneRemoteBundles;
}

export const source: SourceApi = {
  listBundles,
  loadVersion,
  updateVersion,
  resolveFilepath,
  getBuiltinBundleFilepath,
  getRemoteBundleFilepath,
  loadBuiltinMetadata,
  loadRemoteMetadata,
  unloadDescriptor,
  removeRemoteBundle,
  remoteRetainedVersions,
  pruneRemoteBundles,
};
