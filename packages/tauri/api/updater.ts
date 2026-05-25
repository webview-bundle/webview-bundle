import { invoke } from '@tauri-apps/api/core';
import type { ListRemoteBundleInfo, RemoteBundleInfo } from './remote.js';

export type { ListRemoteBundleInfo, RemoteBundleInfo } from './remote.js';

async function listRemotes(): Promise<ListRemoteBundleInfo[]> {
  const remotes = await invoke<ListRemoteBundleInfo[]>('plugin:wvb-tauri|updater_list_remotes');
  return remotes;
}

export interface BundleUpdateInfo {
  name: string;
  version: string;
  localVersion?: string;
  isAvailable: boolean;
  etag?: string;
  integrity?: string;
  signature?: string;
  lastModified?: string;
}

async function getUpdate(bundleName: string): Promise<BundleUpdateInfo> {
  const info = await invoke<BundleUpdateInfo>('plugin:wvb-tauri|updater_get_update', {
    bundleName,
  });
  return info;
}

async function downloadUpdate(bundleName: string, version?: string): Promise<RemoteBundleInfo> {
  const info = await invoke<RemoteBundleInfo>('plugin:wvb-tauri|updater_download_update', {
    bundleName,
    version,
  });
  return info;
}

export interface UpdaterApi {
  listRemotes: typeof listRemotes;
  getUpdate: typeof getUpdate;
  downloadUpdate: typeof downloadUpdate;
}

export const updater: UpdaterApi = {
  listRemotes,
  getUpdate,
  downloadUpdate,
};
