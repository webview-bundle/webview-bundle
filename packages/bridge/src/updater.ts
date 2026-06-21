import { invoke } from './invoke.js';
import type { ListRemoteBundleInfo, RemoteBundleInfo } from './remote.js';

async function listRemotes(): Promise<ListRemoteBundleInfo[]> {
  return invoke<ListRemoteBundleInfo[]>('updaterListRemotes');
}

/** Information of an available bundle update. */
export interface BundleUpdateInfo {
  /** The name of the bundle. */
  name: string;
  /** The latest version available from the remote. */
  version: string;
  /** The version of the bundle currently installed locally. */
  localVersion?: string;
  /** Whether a new update is available. */
  isAvailable: boolean;
  /** "ETag" header value. */
  etag?: string;
  /** Integrity value to verify the bundle. */
  integrity?: string;
  /** Signature value to verify the bundle. */
  signature?: string;
  /** Last modified timestamp. (uses GMT) */
  lastModified?: string;
}

async function getUpdate(bundleName: string): Promise<BundleUpdateInfo> {
  return invoke<BundleUpdateInfo>('updaterGetUpdate', { bundleName });
}

async function download(bundleName: string, version?: string): Promise<RemoteBundleInfo> {
  return invoke<RemoteBundleInfo>('updaterDownload', { bundleName, version });
}

async function install(bundleName: string, version: string): Promise<void> {
  return invoke<void>('updaterInstall', { bundleName, version });
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
