import { invoke } from '@tauri-apps/api/core';

export interface ListRemoteBundleInfo {
  name: string;
  version: string;
}

async function listBundles(channel?: string): Promise<ListRemoteBundleInfo[]> {
  const bundles = await invoke<ListRemoteBundleInfo[]>('plugin:wvb-tauri|remote_list_bundles', {
    channel,
  });
  return bundles;
}

export interface RemoteBundleInfo {
  name: string;
  version: string;
  etag?: string;
  integrity?: string;
  signature?: string;
  lastModified?: string;
}

async function getInfo(bundleName: string, channel?: string): Promise<RemoteBundleInfo> {
  const info = await invoke<RemoteBundleInfo>('plugin:wvb-tauri|remote_get_info', {
    bundleName,
    channel,
  });
  return info;
}

async function download(bundleName: string, channel?: string): Promise<RemoteBundleInfo> {
  const info = await invoke<RemoteBundleInfo>('plugin:wvb-tauri|remote_download', {
    bundleName,
    channel,
  });
  return info;
}

async function downloadVersion(bundleName: string, version: string): Promise<RemoteBundleInfo> {
  const info = await invoke<RemoteBundleInfo>('plugin:wvb-tauri|remote_download_version', {
    bundleName,
    version,
  });
  return info;
}

export interface RemoteApi {
  listBundles: typeof listBundles;
  getInfo: typeof getInfo;
  download: typeof download;
  downloadVersion: typeof downloadVersion;
}

export const remote: RemoteApi = {
  listBundles,
  getInfo,
  download,
  downloadVersion,
};
