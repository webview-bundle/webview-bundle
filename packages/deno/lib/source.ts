// BundleSource — a builtin + remote bundle source, over the native FFI binding.
import { cstr, getLib, readResult } from './ffi.ts';

export interface BundleSourceConfig {
  /** Read-only directory of builtin bundles (`manifest.json` + `<name>/<name>_<version>.wvb`). */
  builtinDir: string;
  /** Writable directory for downloaded (remote) bundles. */
  remoteDir: string;
}

/** Which source a bundle version comes from. */
export type BundleSourceType = 'builtin' | 'remote';

/** Cache-validation / integrity metadata for a bundle version. */
export interface BundleManifestMetadata {
  etag?: string;
  integrity?: string;
  signature?: string;
  lastModified?: string;
}

/** The current version of a bundle and which source provides it. */
export interface BundleSourceVersion {
  type: BundleSourceType;
  version: string;
}

/** A bundle version from {@link BundleSource.listBundles} (flat shape, matching `@wvb/node`). */
export interface ListBundleItem {
  type: BundleSourceType;
  name: string;
  version: string;
  current: boolean;
  metadata: BundleManifestMetadata;
}

/**
 * A bundle source over a `builtinDir` (read-only) and `remoteDir` (writable). Pass it to
 * {@link BundleProtocol}. Free it with `using` or `.free()` once no longer needed (the protocol
 * keeps its own reference, so the source may be freed after the protocol is created).
 *
 * The data methods mirror `@wvb/node`'s `BundleSource` so `@wvb/deno-desktop` can serve the
 * `@wvb/bridge` `source.*` commands.
 */
export class BundleSource {
  #ptr: Deno.PointerValue;

  constructor(config: BundleSourceConfig) {
    const lib = getLib();
    this.#ptr = lib.symbols.wvb_source_new(cstr(config.builtinDir), cstr(config.remoteDir));
    if (this.#ptr === null) {
      throw new Error('wvb: failed to create BundleSource');
    }
  }

  /** @internal Native handle, for passing to a protocol/updater. Throws if already freed. */
  get pointer(): Deno.PointerValue {
    if (this.#ptr === null) {
      throw new Error('wvb: BundleSource has been freed');
    }
    return this.#ptr;
  }

  /** List every bundle version across the builtin + remote directories. */
  async listBundles(): Promise<ListBundleItem[]> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_list_bundles(this.pointer);
    return JSON.parse(readResult(lib, ptr).json) as ListBundleItem[];
  }

  /** The current version for `bundleName` (remote takes priority over builtin), or `null`. */
  async loadVersion(bundleName: string): Promise<BundleSourceVersion | null> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_load_version(this.pointer, cstr(bundleName));
    return JSON.parse(readResult(lib, ptr).json) as BundleSourceVersion | null;
  }

  /** Set the current version of a remote bundle in the manifest. */
  async updateRemoteVersion(bundleName: string, version: string): Promise<void> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_update_version(
      this.pointer,
      cstr(bundleName),
      cstr(version)
    );
    readResult(lib, ptr);
  }

  /** Absolute path to the `.wvb` for the current version (remote over builtin). */
  async resolveFilepath(bundleName: string): Promise<string> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_resolve_filepath(this.pointer, cstr(bundleName));
    return JSON.parse(readResult(lib, ptr).json) as string;
  }

  /** Absolute path to a specific builtin bundle version. */
  getBuiltinBundleFilepath(bundleName: string, version: string): string {
    const lib = getLib();
    const ptr = lib.symbols.wvb_source_get_builtin_filepath(
      this.pointer,
      cstr(bundleName),
      cstr(version)
    );
    return JSON.parse(readResult(lib, ptr).json) as string;
  }

  /** Absolute path to a specific remote bundle version. */
  getRemoteBundleFilepath(bundleName: string, version: string): string {
    const lib = getLib();
    const ptr = lib.symbols.wvb_source_get_remote_filepath(
      this.pointer,
      cstr(bundleName),
      cstr(version)
    );
    return JSON.parse(readResult(lib, ptr).json) as string;
  }

  /** Manifest metadata for a builtin bundle version, or `null`. */
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

  /** Manifest metadata for a remote bundle version, or `null`. */
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

  /** Drop the cached descriptor for a bundle, if present. Returns whether one was removed. */
  unloadDescriptor(bundleName: string): boolean {
    const lib = getLib();
    const ptr = lib.symbols.wvb_source_unload_descriptor(this.pointer, cstr(bundleName));
    return JSON.parse(readResult(lib, ptr).json) as boolean;
  }

  /** Remove a single staged remote bundle version (manifest entry + file). */
  async removeRemoteBundle(bundleName: string, version: string): Promise<boolean> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_remove_remote_bundle(
      this.pointer,
      cstr(bundleName),
      cstr(version)
    );
    return JSON.parse(readResult(lib, ptr).json) as boolean;
  }

  /** The remote versions that pruning retains (current + previous). */
  async remoteRetainedVersions(bundleName: string): Promise<string[]> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_remote_retained_versions(
      this.pointer,
      cstr(bundleName)
    );
    return JSON.parse(readResult(lib, ptr).json) as string[];
  }

  /** Remove every staged remote version except the retained set. Returns the removed versions. */
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
