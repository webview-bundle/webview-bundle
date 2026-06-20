import { invoke } from './invoke.js';

/** List item of remote bundles. */
export interface ListRemoteBundleInfo {
  /** The name of the bundle. */
  name: string;
  /** The version of the bundle. */
  version: string;
}

async function listBundles(channel?: string): Promise<ListRemoteBundleInfo[]> {
  return invoke<ListRemoteBundleInfo[]>('remoteListBundles', { channel });
}

/** Information of a remote bundle. */
export interface RemoteBundleInfo {
  /** The name of the bundle. */
  name: string;
  /** The version of the bundle. */
  version: string;
  /** "ETag" header value. */
  etag?: string;
  /** Integrity value to verify the bundle. */
  integrity?: string;
  /** Signature value to verify the bundle. */
  signature?: string;
  /** Last modified timestamp. (uses GMT) */
  lastModified?: string;
}

async function getInfo(bundleName: string, channel?: string): Promise<RemoteBundleInfo> {
  return invoke<RemoteBundleInfo>('remoteGetInfo', { bundleName, channel });
}

async function download(bundleName: string, channel?: string): Promise<RemoteBundleInfo> {
  return invoke<RemoteBundleInfo>('remoteDownload', { bundleName, channel });
}

async function downloadVersion(bundleName: string, version: string): Promise<RemoteBundleInfo> {
  return invoke<RemoteBundleInfo>('remoteDownloadVersion', { bundleName, version });
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
