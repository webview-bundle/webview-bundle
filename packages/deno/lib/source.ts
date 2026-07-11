import { WebviewBundleError } from './error.ts';
import { cstr, getLib, readResult } from './ffi.ts';

export interface BundleSourceConfig {
  /** Read-only directory of builtin bundles. */
  builtinDir: string;
  /** Writable directory for downloaded remote bundles. */
  remoteDir: string;
}

export type BundleSourceType = 'builtin' | 'remote';

export interface BundleManifestMetadata {
  etag?: string;
  integrity?: string;
  signature?: string;
  lastModified?: string;
}

export interface BundleSourceVersion {
  type: BundleSourceType;
  version: string;
}

export interface ListBundleItem {
  type: BundleSourceType;
  name: string;
  version: string;
  current: boolean;
  metadata: BundleManifestMetadata;
}

export class BundleSource {
  #ptr: Deno.PointerValue;

  constructor(config: BundleSourceConfig) {
    const lib = getLib();
    this.#ptr = lib.symbols.wvb_source_new(cstr(config.builtinDir), cstr(config.remoteDir));
    if (this.#ptr === null) {
      throw new WebviewBundleError('unknown', 'wvb: failed to create BundleSource');
    }
  }

  /** @internal Native handle, for passing to a protocol/updater. Throws if already freed. */
  get pointer(): Deno.PointerValue {
    if (this.#ptr === null) {
      throw new WebviewBundleError('null_handle', 'wvb: BundleSource has been freed');
    }
    return this.#ptr;
  }

  async listBundles(): Promise<ListBundleItem[]> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_list_bundles(this.pointer);
    return JSON.parse(readResult(lib, ptr).json) as ListBundleItem[];
  }

  async loadVersion(bundleName: string): Promise<BundleSourceVersion | null> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_load_version(this.pointer, cstr(bundleName));
    return JSON.parse(readResult(lib, ptr).json) as BundleSourceVersion | null;
  }

  async updateRemoteVersion(bundleName: string, version: string): Promise<void> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_update_version(
      this.pointer,
      cstr(bundleName),
      cstr(version)
    );
    readResult(lib, ptr);
  }

  async resolveFilepath(bundleName: string): Promise<string> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_resolve_filepath(this.pointer, cstr(bundleName));
    return JSON.parse(readResult(lib, ptr).json) as string;
  }

  getBuiltinBundleFilepath(bundleName: string, version: string): string {
    const lib = getLib();
    const ptr = lib.symbols.wvb_source_get_builtin_filepath(
      this.pointer,
      cstr(bundleName),
      cstr(version)
    );
    return JSON.parse(readResult(lib, ptr).json) as string;
  }

  getRemoteBundleFilepath(bundleName: string, version: string): string {
    const lib = getLib();
    const ptr = lib.symbols.wvb_source_get_remote_filepath(
      this.pointer,
      cstr(bundleName),
      cstr(version)
    );
    return JSON.parse(readResult(lib, ptr).json) as string;
  }

  async loadBuiltinMetadata(
    bundleName: string,
    version: string
  ): Promise<BundleManifestMetadata | null> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_load_builtin_metadata(
      this.pointer,
      cstr(bundleName),
      cstr(version)
    );
    return JSON.parse(readResult(lib, ptr).json) as BundleManifestMetadata | null;
  }

  async loadRemoteMetadata(
    bundleName: string,
    version: string
  ): Promise<BundleManifestMetadata | null> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_load_remote_metadata(
      this.pointer,
      cstr(bundleName),
      cstr(version)
    );
    return JSON.parse(readResult(lib, ptr).json) as BundleManifestMetadata | null;
  }

  unloadDescriptor(bundleName: string): boolean {
    const lib = getLib();
    const ptr = lib.symbols.wvb_source_unload_descriptor(this.pointer, cstr(bundleName));
    return JSON.parse(readResult(lib, ptr).json) as boolean;
  }

  async removeRemoteBundle(bundleName: string, version: string): Promise<boolean> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_remove_remote_bundle(
      this.pointer,
      cstr(bundleName),
      cstr(version)
    );
    return JSON.parse(readResult(lib, ptr).json) as boolean;
  }

  async remoteRetainedVersions(bundleName: string): Promise<string[]> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_remote_retained_versions(
      this.pointer,
      cstr(bundleName)
    );
    return JSON.parse(readResult(lib, ptr).json) as string[];
  }

  async pruneRemoteBundles(bundleName: string): Promise<string[]> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_prune_remote_bundles(this.pointer, cstr(bundleName));
    return JSON.parse(readResult(lib, ptr).json) as string[];
  }

  free(): void {
    if (this.#ptr !== null) {
      getLib().symbols.wvb_source_free(this.#ptr);
      this.#ptr = null;
    }
  }

  [Symbol.dispose](): void {
    this.free();
  }
}
