import { invoke } from '@tauri-apps/api/core';

export type BundleSourceType = 'builtin' | 'remote';

export interface BundleManifestMetadata {
  etag?: string;
  integrity?: string;
  signature?: string;
  lastModified?: string;
}

export interface ListBundleManifestItem {
  name: string;
  version: string;
  current: boolean;
  metadata: BundleManifestMetadata;
}

export interface ListBundleItem {
  type: BundleSourceType;
  item: ListBundleManifestItem;
}

async function listBundles(): Promise<ListBundleItem[]> {
  const bundles = await invoke<ListBundleItem[]>('plugin:wvb|source_list_bundles');
  return bundles;
}

export interface BundleSourceVersion {
  type: BundleSourceType;
  version: string;
}

async function loadVersion(bundleName: string): Promise<BundleSourceVersion | null> {
  const version = await invoke<BundleSourceVersion | null>('plugin:wvb|source_load_version', {
    bundleName,
  });
  return version;
}

async function updateVersion(bundleName: string, version: string): Promise<void> {
  await invoke<void>('plugin:wvb|source_update_version', { bundleName, version });
}

async function filepath(bundleName: string): Promise<string> {
  const path = await invoke<string>('plugin:wvb|source_filepath', { bundleName });
  return path;
}

async function getBuiltinBundleFilepath(bundleName: string, version: string): Promise<string> {
  const path = await invoke<string>('plugin:wvb|source_get_builtin_bundle_filepath', {
    bundleName,
    version,
  });
  return path;
}

async function getRemoteBundleFilepath(bundleName: string, version: string): Promise<string> {
  const path = await invoke<string>('plugin:wvb|source_get_remote_bundle_filepath', {
    bundleName,
    version,
  });
  return path;
}

async function loadBuiltinMetadata(
  bundleName: string,
  version: string
): Promise<BundleManifestMetadata | null> {
  const metadata = await invoke<BundleManifestMetadata | null>(
    'plugin:wvb|source_load_builtin_metadata',
    { bundleName, version }
  );
  return metadata;
}

async function loadRemoteMetadata(
  bundleName: string,
  version: string
): Promise<BundleManifestMetadata | null> {
  const metadata = await invoke<BundleManifestMetadata | null>(
    'plugin:wvb|source_load_remote_metadata',
    { bundleName, version }
  );
  return metadata;
}

async function unloadDescriptor(bundleName: string): Promise<boolean> {
  const removed = await invoke<boolean>('plugin:wvb|source_unload_descriptor', { bundleName });
  return removed;
}

async function removeRemoteBundle(bundleName: string, version: string): Promise<boolean> {
  const removed = await invoke<boolean>('plugin:wvb|source_remove_remote_bundle', {
    bundleName,
    version,
  });
  return removed;
}

async function remoteRetainedVersions(bundleName: string): Promise<string[]> {
  const versions = await invoke<string[]>('plugin:wvb|source_remote_retained_versions', {
    bundleName,
  });
  return versions;
}

async function pruneRemoteBundles(bundleName: string): Promise<string[]> {
  const removed = await invoke<string[]>('plugin:wvb|source_prune_remote_bundles', {
    bundleName,
  });
  return removed;
}

export interface SourceApi {
  listBundles: typeof listBundles;
  loadVersion: typeof loadVersion;
  updateVersion: typeof updateVersion;
  filepath: typeof filepath;
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
  filepath,
  getBuiltinBundleFilepath,
  getRemoteBundleFilepath,
  loadBuiltinMetadata,
  loadRemoteMetadata,
  unloadDescriptor,
  removeRemoteBundle,
  remoteRetainedVersions,
  pruneRemoteBundles,
};
