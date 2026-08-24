import type {
  BundleSourceVersion,
  ChecksumReadOptions,
  DataReadOptions,
  HeaderReadOptions,
  IndexReadOptions,
  ManifestBundleItem,
  ManifestBundleItemStatus,
  ManifestPruneResult,
  ManifestRemoveData,
  ManifestRemoveResult,
  ManifestRemoveResultKind,
  ManifestSetCurrentVersionResult,
  ManifestSetCurrentVersionResultKind,
  ManifestStageData,
  ManifestStageResult,
  ManifestStageResultKind,
  ManifestVersionData,
  SourceConfig,
  SourceIntegrityCheckMode,
  SourceIntegrityOptions,
  SourceKind,
  SourceListItem,
  SourceOptions,
} from './bindings.ts';
import { Bundle, BundleDescriptor, LoadedDescriptor } from './bundle.ts';
import {
  cstr,
  getLib,
  readHandle,
  readHandleAsync,
  readJson,
  readJsonAsync,
  requireHandle,
} from './ffi.ts';

export type {
  BundleSourceVersion,
  ChecksumReadOptions,
  DataReadOptions,
  HeaderReadOptions,
  IndexReadOptions,
  ManifestBundleItem,
  ManifestBundleItemStatus,
  ManifestPruneResult,
  ManifestRemoveData,
  ManifestRemoveResult,
  ManifestRemoveResultKind,
  ManifestSetCurrentVersionResult,
  ManifestSetCurrentVersionResultKind,
  ManifestStageData,
  ManifestStageResult,
  ManifestStageResultKind,
  ManifestVersionData,
  SourceConfig,
  SourceIntegrityCheckMode,
  SourceIntegrityOptions,
  SourceKind,
  SourceListItem,
  SourceOptions,
};

/** The format version of a `manifest.json`. */
export type ManifestVersion = 1;

/** Every version of one bundle a manifest records, plus which of them are in play. */
export interface ManifestBundleSet {
  versions: Record<string, ManifestVersionData>;
  currentVersion?: string;
  previousVersion?: string;
  stagedVersion?: string;
}

/** A `manifest.json` as it is stored on disk. */
export interface ManifestData {
  manifestVersion: ManifestVersion;
  bundles: Record<string, ManifestBundleSet>;
}

/**
 * The local bundle store: read-only builtin bundles shipped with the app, plus a writable directory
 * of downloaded remote ones. A remote version takes priority over the builtin of the same name.
 *
 * Owns a native handle — call {@link Source.free} (or `using source = new Source(...)`) when done.
 */
export class Source {
  #ptr: Deno.PointerValue;

  constructor(config: SourceConfig) {
    const lib = getLib();
    // An ill-formed option (a misspelled `integrity.checkMode`, an unknown policy) fails here
    // rather than leaving verification off while the caller believes it is on.
    this.#ptr = readHandle(lib, lib.symbols.wvb_source_new(cstr(JSON.stringify(config))));
  }

  /** @internal Native handle, for passing to a protocol/updater. Throws if already freed. */
  get pointer(): Deno.PointerValue {
    return requireHandle(this.#ptr, 'Source');
  }

  listBundles(): Promise<SourceListItem[]> {
    const lib = getLib();
    return readJsonAsync(lib.symbols.wvb_source_list_bundles(this.pointer));
  }

  listBuiltinBundles(): Promise<SourceListItem[]> {
    const lib = getLib();
    return readJsonAsync(lib.symbols.wvb_source_list_builtin_bundles(this.pointer));
  }

  listRemoteBundles(): Promise<SourceListItem[]> {
    const lib = getLib();
    return readJsonAsync(lib.symbols.wvb_source_list_remote_bundles(this.pointer));
  }

  /** The version currently served for `bundleName` — remote first, then builtin. */
  getVersion(bundleName: string): Promise<BundleSourceVersion | null> {
    const lib = getLib();
    return readJsonAsync(lib.symbols.wvb_source_get_version(this.pointer, cstr(bundleName)));
  }

  getRemoteStagedVersion(bundleName: string): Promise<string | null> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_source_get_remote_staged_version(this.pointer, cstr(bundleName))
    );
  }

  getRemotePreviousVersion(bundleName: string): Promise<string | null> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_source_get_remote_previous_version(this.pointer, cstr(bundleName))
    );
  }

  /** Activates `version` for `bundleName`; the version has to be recorded in the manifest. */
  updateRemoteVersion(
    bundleName: string,
    version: string
  ): Promise<ManifestSetCurrentVersionResult> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_source_update_remote_version(this.pointer, cstr(bundleName), cstr(version))
    );
  }

  /** Same as {@link Source.updateRemoteVersion} for several bundles, in one manifest write. */
  updateRemoteVersions(items: Record<string, string>): Promise<ManifestSetCurrentVersionResult[]> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_source_update_remote_versions(this.pointer, cstr(JSON.stringify(items)))
    );
  }

  /**
   * Records a downloaded version in the manifest without activating it. Write the `.wvb` file to
   * {@link Source.getRemoteBundleFilepath} first; {@link Source.updateRemoteVersion} activates it.
   */
  stageRemoteBundle(bundleName: string, data: ManifestStageData): Promise<ManifestStageResult> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_source_stage_remote_bundle(
        this.pointer,
        cstr(bundleName),
        cstr(JSON.stringify(data))
      )
    );
  }

  /** Same as {@link Source.stageRemoteBundle} for several bundles, in one manifest write. */
  stageRemoteBundles(items: Record<string, ManifestStageData>): Promise<ManifestStageResult[]> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_source_stage_remote_bundles(this.pointer, cstr(JSON.stringify(items)))
    );
  }

  /** The `.wvb` file backing the version currently served for `bundleName`. */
  resolveFilepath(bundleName: string): Promise<string> {
    const lib = getLib();
    return readJsonAsync(lib.symbols.wvb_source_resolve_filepath(this.pointer, cstr(bundleName)));
  }

  getBuiltinBundleFilepath(bundleName: string, version: string): string {
    const lib = getLib();
    return readJson(
      lib.symbols.wvb_source_get_builtin_bundle_filepath(
        this.pointer,
        cstr(bundleName),
        cstr(version)
      )
    );
  }

  getRemoteBundleFilepath(bundleName: string, version: string): string {
    const lib = getLib();
    return readJson(
      lib.symbols.wvb_source_get_remote_bundle_filepath(
        this.pointer,
        cstr(bundleName),
        cstr(version)
      )
    );
  }

  getBuiltinVersionData(bundleName: string, version: string): Promise<ManifestVersionData | null> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_source_get_builtin_version_data(this.pointer, cstr(bundleName), cstr(version))
    );
  }

  getRemoteVersionData(bundleName: string, version: string): Promise<ManifestVersionData | null> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_source_get_remote_version_data(this.pointer, cstr(bundleName), cstr(version))
    );
  }

  /** Fetches and fully loads the current version of a bundle into memory. */
  async fetchBundle(bundleName: string): Promise<Bundle> {
    const lib = getLib();
    return new Bundle(
      await readHandleAsync(lib.symbols.wvb_source_fetch_bundle(this.pointer, cstr(bundleName)))
    );
  }

  /** Fetches and fully loads a specific builtin bundle version into memory. */
  async fetchBuiltinBundle(bundleName: string, version: string): Promise<Bundle> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_fetch_builtin_bundle(
      this.pointer,
      cstr(bundleName),
      cstr(version)
    );
    return new Bundle(readHandle(lib, ptr));
  }

  /** Fetches and fully loads a specific remote bundle version into memory. */
  async fetchRemoteBundle(bundleName: string, version: string): Promise<Bundle> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_fetch_remote_bundle(
      this.pointer,
      cstr(bundleName),
      cstr(version)
    );
    return new Bundle(readHandle(lib, ptr));
  }

  /**
   * Fetches the descriptor (header + index, no data) for the current version. Read entry data
   * lazily via {@link BundleDescriptor.getData}, passing a filepath (e.g. from
   * {@link Source.resolveFilepath}).
   */
  async fetchDescriptor(bundleName: string): Promise<BundleDescriptor> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_fetch_descriptor(this.pointer, cstr(bundleName));
    return new BundleDescriptor(readHandle(lib, ptr));
  }

  /**
   * Loads (and caches) the descriptor for the current version. The returned
   * {@link LoadedDescriptor} remembers its filepath + read options and keeps working across
   * active-version swaps; {@link Source.unload} drops the cache entry.
   */
  async load(bundleName: string): Promise<LoadedDescriptor> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_load(this.pointer, cstr(bundleName));
    return new LoadedDescriptor(readHandle(lib, ptr));
  }

  /** Drops the cached descriptor for `bundleName`. Returns `true` when one was cached. */
  unload(bundleName: string): boolean {
    const lib = getLib();
    return readJson(lib.symbols.wvb_source_unload(this.pointer, cstr(bundleName)));
  }

  /** Removes a downloaded version and its file. The version in use is kept unless `force`. */
  removeRemoteBundle(
    bundleName: string,
    version: string,
    force?: boolean
  ): Promise<ManifestRemoveResult> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_source_remove_remote_bundle(
        this.pointer,
        cstr(bundleName),
        cstr(version),
        force == null ? -1 : force ? 1 : 0
      )
    );
  }

  /** Same as {@link Source.removeRemoteBundle} for several bundles, in one manifest write. */
  removeRemoteBundles(items: Record<string, ManifestRemoveData>): Promise<ManifestRemoveResult[]> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_source_remove_remote_bundles(this.pointer, cstr(JSON.stringify(items)))
    );
  }

  /** Removes the downloaded versions of `bundleName` that are no longer referenced. */
  pruneRemoteBundle(bundleName: string): Promise<ManifestPruneResult> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_source_prune_remote_bundle(this.pointer, cstr(bundleName))
    );
  }

  /** Same as {@link Source.pruneRemoteBundle} for several bundles, in one manifest write. */
  pruneRemoteBundles(bundleNames: string[]): Promise<ManifestPruneResult[]> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_source_prune_remote_bundles(this.pointer, cstr(JSON.stringify(bundleNames)))
    );
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
