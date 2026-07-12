import { WebviewBundleError } from './error.ts';
import { cstr, getLib, readResult } from './ffi.ts';
import {
  type IntegrityPolicy,
  type SignatureVerifierOptions,
  serializeSignatureVerifier,
} from './updater.ts';

/**
 * Which bundles are verified against the integrity/signature recorded in their manifest when
 * they are loaded from disk.
 *
 * A bundle is verified once per version, when it is first read, so serving it does not re-hash
 * it on every request.
 *
 * - `'none'` (default) — never verify on load; bundles are still verified when downloaded.
 * - `'remote'` — verify downloaded bundles only. Builtin bundles carry integrity metadata only
 *   if the app was packed with it, so this is the setting that works without changing how
 *   builtin bundles are built.
 * - `'all'` — also verify builtin bundles, which then must carry integrity metadata.
 */
export type VerifyOnLoad = 'none' | 'remote' | 'all';

export interface BundleSourceConfig {
  /** Read-only directory of builtin bundles. */
  builtinDir: string;
  /** Writable directory for downloaded remote bundles. */
  remoteDir: string;
  /** Which bundles are verified when loaded from disk. Default: `'none'`. */
  verifyOnLoad?: VerifyOnLoad;
  /** How a missing or mismatched integrity is treated on load. */
  integrityPolicy?: IntegrityPolicy;
  /**
   * Verify that a bundle's integrity string was signed by the matching key.
   *
   * The signature signs the integrity string, so setting this also makes the integrity check
   * mandatory whatever {@link BundleSourceConfig.integrityPolicy} says — a signature over an
   * unchecked hash proves nothing about the bundle's bytes.
   */
  signatureVerifier?: SignatureVerifierOptions;
  /**
   * Verify each entry's xxHash-32 checksum when its data is read through this source.
   * Default: `false`. ({@link BundleProtocol} verifies by default and overrides this.)
   */
  verifyDataChecksum?: boolean;
  /** The seed the bundle's data checksums were built with. Default: `0`. */
  dataChecksumSeed?: number;
}

function serializeConfig(config: BundleSourceConfig): string {
  const { builtinDir: _builtin, remoteDir: _remote, signatureVerifier, ...options } = config;
  return JSON.stringify(
    signatureVerifier == null
      ? options
      : { ...options, signatureVerifier: serializeSignatureVerifier(signatureVerifier) }
  );
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
    this.#ptr = lib.symbols.wvb_source_new_with_options(
      cstr(config.builtinDir),
      cstr(config.remoteDir),
      cstr(serializeConfig(config))
    );
    if (this.#ptr === null) {
      // A null source also means an option was ill-formed — e.g. a `signatureVerifier` that
      // couldn't be built. Fail closed rather than read bundles unverified.
      throw new WebviewBundleError(
        'invalid_signature_options',
        'wvb: failed to create BundleSource (check verifyOnLoad/integrityPolicy/signatureVerifier)'
      );
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
