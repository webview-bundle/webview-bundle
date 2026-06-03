import { invoke } from '@tauri-apps/api/core';
import type { ListRemoteBundleInfo, RemoteBundleInfo } from './remote.js';

export type { ListRemoteBundleInfo, RemoteBundleInfo } from './remote.js';

async function listRemotes(): Promise<ListRemoteBundleInfo[]> {
  const remotes = await invoke<ListRemoteBundleInfo[]>('plugin:wvb|updater_list_remotes');
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
  const info = await invoke<BundleUpdateInfo>('plugin:wvb|updater_get_update', {
    bundleName,
  });
  return info;
}

async function download(bundleName: string, version?: string): Promise<RemoteBundleInfo> {
  const info = await invoke<RemoteBundleInfo>('plugin:wvb|updater_download', {
    bundleName,
    version,
  });
  return info;
}

async function install(bundleName: string, version: string): Promise<void> {
  await invoke<void>('plugin:wvb|updater_install', { bundleName, version });
}

export interface UpdaterApi {
  listRemotes: typeof listRemotes;
  getUpdate: typeof getUpdate;
  download: typeof download;
  install: typeof install;
}

export const updater: UpdaterApi = {
  listRemotes,
  getUpdate,
  download,
  install,
};
