import { invoke } from './invoke.js';
import type { BundleUpdate, Update } from './remote.js';

export interface UpdaterGetUpdateOptions {
  /**
   * Require the update document to be signed by the key published under this id. The key itself is
   * configured on the host, not here.
   */
  expectSignatureKeyId?: string;
}

export interface UpdaterDownloadOptions {
  /** How many bundles are downloaded at once. Default: `3`. */
  concurrency?: number;
  /** How long to wait for the updater lock, in milliseconds. */
  timeout?: number;
}

/** The outcome of downloading one bundle. */
export type UpdaterDownloadResultKind =
  | { type: 'downloaded' }
  | { type: 'error'; code: string; message: string };

export interface UpdaterDownloadResult {
  name: string;
  version: string;
  integrity?: string;
  metadata?: Record<string, string>;
  result: UpdaterDownloadResultKind;
}

export interface UpdaterInstallTarget {
  name: string;
  /**
   * The staged version to install. When omitted, the staged version recorded in the manifest is
   * used; when given, it has to match that staged version.
   */
  version?: string;
}

/** The outcome of installing one staged bundle. */
export type UpdaterInstallResultKind =
  | { type: 'installed' }
  | { type: 'staged_version_not_matched' }
  | { type: 'staged_bundle_not_exists' }
  | { type: 'verify_failed' }
  | { type: 'error'; code: string; message: string };

export interface UpdaterInstallResult {
  name: string;
  targetVersion?: string;
  installVersion?: string;
  result: UpdaterInstallResultKind;
}

export interface UpdaterRollbackTarget {
  name: string;
  /**
   * The previous version to roll back to. When omitted, the previous version recorded in the
   * manifest is used; when given, it has to match that previous version.
   */
  version?: string;
}

/** The outcome of rolling one bundle back to its previous version. */
export type UpdaterRollbackResultKind =
  | { type: 'rolled_back' }
  | { type: 'previous_version_not_matched' }
  | { type: 'previous_bundle_not_exists' }
  | { type: 'verify_failed' }
  | { type: 'error'; code: string; message: string };

export interface UpdaterRollbackResult {
  name: string;
  targetVersion?: string;
  rollbackVersion?: string;
  result: UpdaterRollbackResultKind;
}

/**
 * The bundles the app is missing, or `null` when it is already up to date.
 */
async function getUpdate(options?: UpdaterGetUpdateOptions): Promise<Update | null> {
  return invoke<Update | null>('updaterGetUpdate', { options });
}

/**
 * Downloads the given bundle updates. This only stages them on disk — {@link UpdaterApi.install}
 * is what activates them for the protocol to serve.
 *
 * A bundle that failed to download is reported in its own result rather than failing the call, so
 * check each `result.type`.
 */
async function download(
  bundleUpdates: BundleUpdate[],
  options?: UpdaterDownloadOptions
): Promise<UpdaterDownloadResult[]> {
  return invoke<UpdaterDownloadResult[]>('updaterDownload', { bundleUpdates, options });
}

/** Activates the staged version of each target. */
async function install(targets: UpdaterInstallTarget[]): Promise<UpdaterInstallResult[]> {
  return invoke<UpdaterInstallResult[]>('updaterInstall', { targets });
}

/** Puts each target back on the previous version recorded for it. */
async function rollback(targets: UpdaterRollbackTarget[]): Promise<UpdaterRollbackResult[]> {
  return invoke<UpdaterRollbackResult[]>('updaterRollback', { targets });
}

export interface UpdaterApi {
  getUpdate: typeof getUpdate;
  download: typeof download;
  install: typeof install;
  rollback: typeof rollback;
}

export const updater: UpdaterApi = {
  getUpdate,
  download,
  install,
  rollback,
};
