// Updater — coordinates updates between a BundleSource and a Remote.
import { cstr, getLib, readResult } from './ffi.ts';
import type { ListRemoteBundleInfo, Remote, RemoteBundleInfo } from './remote.ts';
import type { BundleSource } from './source.ts';

/** Integrity verification policy. */
export type IntegrityPolicy = 'strict' | 'optional' | 'none';

export interface UpdaterOptions {
  channel?: string;
  integrityPolicy?: IntegrityPolicy;
}

/** Information about an available update. */
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

/**
 * Coordinates updates between a {@link BundleSource} and a {@link Remote}: check, download to the
 * remote dir, and activate.
 *
 * Note: `signatureVerifier` / custom `integrityChecker` (callback options in `@wvb/node`) are not
 * yet supported; `channel` and `integrityPolicy` are.
 */
export class Updater {
  #ptr: Deno.PointerValue;

  constructor(source: BundleSource, remote: Remote, options?: UpdaterOptions) {
    const lib = getLib();
    this.#ptr = lib.symbols.wvb_updater_new(
      source.pointer,
      remote.pointer,
      cstr(options != null ? JSON.stringify(options) : '')
    );
    if (this.#ptr === null) {
      throw new Error('wvb: failed to create Updater');
    }
  }

  async listRemotes(): Promise<ListRemoteBundleInfo[]> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_updater_list_remotes(this.#ptr);
    return JSON.parse(readResult(lib, ptr).json) as ListRemoteBundleInfo[];
  }

  async getUpdate(bundleName: string): Promise<BundleUpdateInfo> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_updater_get_update(this.#ptr, cstr(bundleName));
    return JSON.parse(readResult(lib, ptr).json) as BundleUpdateInfo;
  }

  async download(bundleName: string, version?: string): Promise<RemoteBundleInfo> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_updater_download(
      this.#ptr,
      cstr(bundleName),
      cstr(version ?? '')
    );
    return JSON.parse(readResult(lib, ptr).json) as RemoteBundleInfo;
  }

  async install(bundleName: string, version: string): Promise<void> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_updater_install(this.#ptr, cstr(bundleName), cstr(version));
    readResult(lib, ptr);
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
