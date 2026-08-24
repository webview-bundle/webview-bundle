import type {
  BundleUpdate,
  IntegrityAlgorithm,
  IntegrityPolicy,
  Update,
  UpdaterDownloadOptions,
  UpdaterGetUpdateOptions,
  UpdaterInstallTarget,
  UpdaterIntegrityOptions,
  UpdaterRollbackTarget,
} from './bindings.ts';
import type { Cancellation } from './cancellation.ts';
import type { ErrorCode } from './error-codes.ts';
import { cstr, getLib, readHandle, readJsonAsync, requireHandle } from './ffi.ts';
import type { Remote } from './remote.ts';
import { type SignatureVerifyKey, serializeSignatureVerifyKey } from './signature.ts';
import type { Source } from './source.ts';

export type {
  IntegrityAlgorithm,
  IntegrityPolicy,
  UpdaterDownloadOptions,
  UpdaterGetUpdateOptions,
  UpdaterInstallTarget,
  UpdaterIntegrityOptions,
  UpdaterRollbackTarget,
};

/**
 * The keys an update response may be signed with, each published under its own id.
 *
 * The signature signs the update document, not the bundle bytes; keep
 * {@link UpdaterIntegrityOptions.policy} enabled (not `'off'`) so the integrity strings it carries
 * also authenticate what is downloaded.
 */
export interface UpdaterSignatureOptions {
  keys?: SignatureVerifyKey[];
}

export interface UpdaterOptions {
  /** Fetch updates from this release channel. */
  channel?: string;
  /** How a downloaded bundle is checked against the integrity the update advertises. */
  integrity?: UpdaterIntegrityOptions;
  /** Recommended in production: verify the update document before acting on it. */
  signature?: UpdaterSignatureOptions;
}

/** The outcome of downloading one bundle. */
export type UpdaterDownloadResultKind =
  | { type: 'downloaded' }
  | { type: 'error'; code: ErrorCode; message: string };

export interface UpdaterDownloadResult {
  name: string;
  version: string;
  integrity?: string;
  metadata?: Record<string, string>;
  result: UpdaterDownloadResultKind;
}

/** The outcome of installing one staged bundle. */
export type UpdaterInstallResultKind =
  | { type: 'installed' }
  | { type: 'staged_version_not_matched' }
  | { type: 'staged_bundle_not_exists' }
  | { type: 'verify_failed' }
  | { type: 'error'; code: ErrorCode; message: string };

export interface UpdaterInstallResult {
  name: string;
  targetVersion?: string;
  installVersion?: string;
  result: UpdaterInstallResultKind;
}

/** The outcome of rolling one bundle back to its previous version. */
export type UpdaterRollbackResultKind =
  | { type: 'rolled_back' }
  | { type: 'previous_version_not_matched' }
  | { type: 'previous_bundle_not_exists' }
  | { type: 'verify_failed' }
  | { type: 'error'; code: ErrorCode; message: string };

export interface UpdaterRollbackResult {
  name: string;
  targetVersion?: string;
  rollbackVersion?: string;
  result: UpdaterRollbackResultKind;
}

function serializeOptions(options: UpdaterOptions): string {
  const { signature, ...rest } = options;
  if (signature?.keys == null) {
    return JSON.stringify(options);
  }
  return JSON.stringify({
    ...rest,
    signature: { ...signature, keys: signature.keys.map(serializeSignatureVerifyKey) },
  });
}

/**
 * Drives the update cycle over a {@link Source} and a {@link Remote}: ask what is available,
 * download it, then install (or roll back) what was downloaded.
 *
 * Owns a native handle — call {@link Updater.free} (or `using updater = new Updater(...)`) when done.
 */
export class Updater {
  #ptr: Deno.PointerValue;

  constructor(source: Source, remote: Remote, updateFilepath: string, options?: UpdaterOptions) {
    const lib = getLib();
    // A key the native side cannot build fails here rather than serving updates unverified.
    this.#ptr = readHandle(
      lib,
      lib.symbols.wvb_updater_new(
        source.pointer,
        remote.pointer,
        cstr(updateFilepath),
        cstr(options != null ? serializeOptions(options) : '')
      )
    );
  }

  /** @internal Native handle. Throws if already freed. */
  get pointer(): Deno.PointerValue {
    return requireHandle(this.#ptr, 'Updater');
  }

  /** The bundles this source is missing, or `null` when it is already up to date. */
  getUpdate(options?: UpdaterGetUpdateOptions): Promise<Update | null> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_updater_get_update(
        this.pointer,
        cstr(options != null ? JSON.stringify(options) : '')
      )
    );
  }

  /**
   * Downloads the given bundle updates. This only stages them on disk — {@link Updater.install} is
   * what activates them for the protocol to serve.
   */
  download(
    bundleUpdates: BundleUpdate[],
    options?: UpdaterDownloadOptions,
    cancellation?: Cancellation
  ): Promise<UpdaterDownloadResult[]> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_updater_download(
        this.pointer,
        cstr(JSON.stringify(bundleUpdates)),
        cstr(options != null ? JSON.stringify(options) : ''),
        cancellation?.pointer ?? null
      )
    );
  }

  /** Activates the staged version of each target. */
  install(targets: UpdaterInstallTarget[]): Promise<UpdaterInstallResult[]> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_updater_install(this.pointer, cstr(JSON.stringify(targets)))
    );
  }

  /** Puts each target back on the previous version recorded for it. */
  rollback(targets: UpdaterRollbackTarget[]): Promise<UpdaterRollbackResult[]> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_updater_rollback(this.pointer, cstr(JSON.stringify(targets)))
    );
  }

  free(): void {
    if (this.#ptr !== null) {
      getLib().symbols.wvb_updater_free(this.#ptr);
      this.#ptr = null;
    }
  }

  [Symbol.dispose](): void {
    this.free();
  }
}
