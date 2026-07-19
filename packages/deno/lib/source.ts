import type {
  BundleManifestMetadata,
  BundleSourceType,
  BundleSourceVerifyMode,
  BundleSourceVersion,
  ListBundleItem,
} from './bindings.ts';
import { Bundle, BundleDescriptor, LoadedDescriptor } from './bundle.ts';
import { WebviewBundleError } from './error.ts';
import { cstr, getLib, readHandle, readResult } from './ffi.ts';
import {
  type IntegrityPolicy,
  type SignatureVerifierOptions,
  serializeSignatureVerifier,
} from './updater.ts';

export type {
  BundleManifestMetadata,
  BundleSourceType,
  BundleSourceVerifyMode,
  BundleSourceVersion,
  ListBundleItem,
};

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

/**
 * How a bundle section's xxHash checksum is verified when that section is read through this
 * source. The same options apply to the header, the index and each entry's data.
 *
 * This detects corruption, not tampering: the seed is not secret, so whatever can rewrite the
 * bytes can rewrite the checksum. Use {@link BundleSourceConfig.signature} to detect tampering.
 */
export interface ChecksumReadOptions {
  /** Verify the section's checksum when it is read. Default: `true`. */
  verify?: boolean;
  /** The seed the checksum was built with. Default: `0`. */
  seed?: number;
}

/** How each entry's data is read out of a bundle's data section. */
export interface DataReadOptions {
  /** How the data checksum is verified. */
  checksum?: ChecksumReadOptions;
}

/** How a bundle's header is read. */
export interface HeaderReadOptions {
  /** How the header checksum is verified. */
  checksum?: ChecksumReadOptions;
}

/** How a bundle's index is read. */
export interface IndexReadOptions {
  /** How the index checksum is verified. */
  checksum?: ChecksumReadOptions;
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
   * How each entry's data checksum is verified when its data is read through this source.
   * Default: `{ checksum: { verify: true, seed: 0 } }`. ({@link BundleProtocol} verifies by default
   * too, and overrides this.)
   */
  dataReadOptions?: DataReadOptions;
  /**
   * How a bundle's header checksum is verified when its descriptor is read on load.
   * Default: `{ checksum: { verify: true, seed: 0 } }`.
   */
  headerReadOptions?: HeaderReadOptions;
  /**
   * How a bundle's index checksum is verified when its descriptor is read on load.
   * Default: `{ checksum: { verify: true, seed: 0 } }`.
   */
  indexReadOptions?: IndexReadOptions;
}

const CONFIG_KEYS: ReadonlySet<string> = new Set([
  'builtinDir',
  'remoteDir',
  'integrity',
  'signature',
  'dataReadOptions',
  'headerReadOptions',
  'indexReadOptions',
]);
const INTEGRITY_KEYS: ReadonlySet<string> = new Set(['policy', 'checkMode']);
const SIGNATURE_KEYS: ReadonlySet<string> = new Set(['verify', 'verifyMode']);
const READ_OPTION_KEYS: ReadonlySet<string> = new Set(['checksum']);
const READ_CHECKSUM_KEYS: ReadonlySet<string> = new Set(['verify', 'seed']);
const READ_OPTION_GROUPS = ['dataReadOptions', 'headerReadOptions', 'indexReadOptions'] as const;

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
  for (const group of READ_OPTION_GROUPS) {
    const value = config[group];
    if (value != null) {
      assertKnownKeys(`BundleSource ${group}`, value, READ_OPTION_KEYS);
      if (value.checksum != null) {
        assertKnownKeys(`BundleSource ${group}.checksum`, value.checksum, READ_CHECKSUM_KEYS);
      }
    }
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
            'wvb: failed to create BundleSource (check integrity/signature/dataReadOptions/headerReadOptions/indexReadOptions)'
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

  /** Fetches and fully loads the current version of a bundle into memory. */
  async fetchBundle(bundleName: string): Promise<Bundle> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_fetch_bundle(this.pointer, cstr(bundleName));
    return new Bundle(readHandle(lib, ptr));
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
   * {@link BundleSource.resolveFilepath}).
   */
  async fetchDescriptor(bundleName: string): Promise<BundleDescriptor> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_fetch_descriptor(this.pointer, cstr(bundleName));
    return new BundleDescriptor(readHandle(lib, ptr));
  }

  /**
   * Loads (and caches) the descriptor for the current version. The returned
   * {@link LoadedDescriptor} remembers its filepath + read options and keeps working across
   * active-version swaps; {@link BundleSource.unloadDescriptor} drops the cache entry.
   */
  async loadDescriptor(bundleName: string): Promise<LoadedDescriptor> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_load_descriptor(this.pointer, cstr(bundleName));
    return new LoadedDescriptor(readHandle(lib, ptr));
  }

  /**
   * Persists the raw bytes of a `.wvb` file to the remote directory and records `metadata` in the
   * manifest. Prefer this over re-serializing a parsed {@link Bundle} when the bytes are already at
   * hand (e.g. a {@link Remote} download): storing them verbatim keeps the integrity string valid on
   * later loads. The version is staged, not activated — call {@link BundleSource.updateRemoteVersion}.
   */
  async writeRemoteBundleData(
    bundleName: string,
    version: string,
    data: Uint8Array<ArrayBuffer>,
    metadata: BundleManifestMetadata = {}
  ): Promise<void> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_source_write_remote_bundle_data(
      this.pointer,
      cstr(bundleName),
      cstr(version),
      data,
      BigInt(data.byteLength),
      cstr(JSON.stringify(metadata))
    );
    readResult(lib, ptr);
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
