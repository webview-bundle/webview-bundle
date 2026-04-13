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

export interface SourceApi {
  listBundles: typeof listBundles;
  loadVersion: typeof loadVersion;
  updateVersion: typeof updateVersion;
  filepath: typeof filepath;
}

export const source: SourceApi = {
  listBundles,
  loadVersion,
  updateVersion,
  filepath,
};
