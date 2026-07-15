import { WebviewBundleError } from './error.ts';
import { cstr, getLib, readResult } from './ffi.ts';
import {
  type IntegrityPolicy,
  type SignatureVerifierOptions,
  serializeSignatureVerifier,
} from './updater.ts';

/**
 * Which bundles a load-time verification applies to.
 *
 * A bundle is verified once per version, when it is first read, so serving it does not re-hash
 * it on every request.
 *
 * - `'onlyRemote'` (default) — verify downloaded bundles only. Builtin bundles carry the
 *   metadata being verified only if the app was packed with it, so this is the setting that
 *   works without changing how builtin bundles are built.
 * - `'all'` — also verify builtin bundles, which requires the builtin manifest to carry the
 *   metadata being verified.
 */
export type BundleSourceVerifyMode = 'all' | 'onlyRemote';

/**
 * How bundles are checked against the integrity recorded for them in the manifest when they are
 * loaded from disk.
 */
export interface BundleSourceIntegrityOptions {
  /**
   * How a bundle's integrity metadata is treated. Default: `'optional'` — checked when present,
   * tolerated when missing. `'strict'` requires it; `'off'` disables the check entirely.
   */
  policy?: IntegrityPolicy;
  /** Which bundles are checked on load. Default: `'onlyRemote'`. */
  checkMode?: BundleSourceVerifyMode;
}

/**
 * How bundle signatures are verified when bundles are loaded from disk.
 *
 * A bundle's signature signs its integrity string, not the bundle bytes; verifying it proves the
 * integrity string is authentic. It is verified independently of the integrity check, so pair it
 * with an enabled {@link BundleSourceConfig.integrity} to also authenticate the bytes — signature
 * verification alone does not read them.
 */
export interface BundleSourceSignatureOptions {
  /** Verify that a bundle's integrity string was signed by the matching key. Default: off. */
  verify?: SignatureVerifierOptions;
  /** Which bundles have their signature verified on load. Default: `'onlyRemote'`. */
  verifyMode?: BundleSourceVerifyMode;
}

export interface BundleSourceConfig {
  /** Read-only directory of builtin bundles. */
  builtinDir: string;
  /** Writable directory for downloaded remote bundles. */
  remoteDir: string;
  /** How bundles are checked against their manifest integrity metadata on load. */
  integrity?: BundleSourceIntegrityOptions;
  /** How bundle signatures are verified on load. */
  signature?: BundleSourceSignatureOptions;
  /**
   * Verify each entry's xxHash-32 checksum when its data is read through this source.
   * Default: `true`. ({@link BundleProtocol} verifies by default too, and overrides this.)
   *
   * This detects corruption, not tampering: the seed is not secret, so whatever can rewrite an
   * entry can rewrite its checksum. Use {@link BundleSourceConfig.signature} to detect
   * tampering.
   */
  verifyDataChecksum?: boolean;
  /** The seed the bundle's data checksums were built with. Default: `0`. */
  dataChecksumSeed?: number;
}

const CONFIG_KEYS: ReadonlySet<string> = new Set([
  'builtinDir',
  'remoteDir',
  'integrity',
  'signature',
  'verifyDataChecksum',
  'dataChecksumSeed',
]);
const INTEGRITY_KEYS: ReadonlySet<string> = new Set(['policy', 'checkMode']);
const SIGNATURE_KEYS: ReadonlySet<string> = new Set(['verify', 'verifyMode']);

function assertKnownKeys(what: string, value: object, known: ReadonlySet<string>): void {
  for (const key of Object.keys(value)) {
    if (!known.has(key)) {
      throw new WebviewBundleError('unknown', `wvb: unknown ${what} option '${key}'`);
    }
  }
}

function serializeConfig(config: BundleSourceConfig): string {
  // A misspelled security option (`integrity.checkmode`) would otherwise be dropped in silence,
  // leaving verification off while the caller believes it is on.
  assertKnownKeys('BundleSource', config, CONFIG_KEYS);
  const { builtinDir: _builtin, remoteDir: _remote, integrity, signature, ...options } = config;
  if (integrity != null) {
    assertKnownKeys('BundleSource integrity', integrity, INTEGRITY_KEYS);
  }
  if (signature != null) {
    assertKnownKeys('BundleSource signature', signature, SIGNATURE_KEYS);
  }
  return JSON.stringify({
    ...options,
    ...(integrity != null ? { integrity } : {}),
    ...(signature != null
      ? {
          signature:
            signature.verify == null
              ? signature
              : { ...signature, verify: serializeSignatureVerifier(signature.verify) },
        }
      : {}),
  });
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
      // A null source means an option was ill-formed — an `integrity`/checksum value the native
      // side rejected, or a `signature.verify` key it couldn't build. Fail closed rather than
      // read bundles unverified; only blame the key when one was actually given.
      throw config.signature?.verify != null
        ? new WebviewBundleError(
            'invalid_signature_options',
            'wvb: failed to create BundleSource (check integrity/signature.verify)'
          )
        : new WebviewBundleError(
            'unknown',
            'wvb: failed to create BundleSource (check integrity/signature/verifyDataChecksum/dataChecksumSeed)'
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
