// Updater — coordinates updates between a BundleSource and a Remote.
import { encodeBase64 } from '@std/encoding/base64';
import { cstr, getLib, readResult } from './ffi.ts';
import type { ListRemoteBundleInfo, Remote, RemoteBundleInfo } from './remote.ts';
import type { BundleSource } from './source.ts';

/** Integrity verification policy. */
export type IntegrityPolicy = 'strict' | 'optional' | 'none';

/** Digital signature algorithm for bundle verification (mirrors `@wvb/node`). */
export type SignatureAlgorithm =
  | 'ecdsaSecp256R1'
  | 'ecdsaSecp384R1'
  | 'ed25519'
  | 'rsaPkcs1V15'
  | 'rsaPss';

/** Format of the public key. Binary formats (`spkiDer`/`pkcs1Der`/`sec1`/`raw`) take `Uint8Array`
 * data; the PEM formats take the PEM text. `pkcs1*` is RSA-only, `sec1` is ECDSA-only, `raw` is
 * Ed25519-only (32 bytes). */
export type VerifyingKeyFormat = 'spkiDer' | 'spkiPem' | 'pkcs1Der' | 'pkcs1Pem' | 'sec1' | 'raw';

/** Public key configuration: `data` is the PEM text for PEM formats, or the key bytes otherwise. */
export interface SignatureVerifyingKeyOptions {
  format: VerifyingKeyFormat;
  data: string | Uint8Array;
}

/** Declarative signature verifier: an algorithm + the public key to verify bundle signatures with. */
export interface SignatureVerifierOptions {
  algorithm: SignatureAlgorithm;
  key: SignatureVerifyingKeyOptions;
}

export interface UpdaterOptions {
  channel?: string;
  integrityPolicy?: IntegrityPolicy;
  /**
   * Verify bundle signatures against a public key. Mirrors `@wvb/node`'s declarative
   * `signatureVerifier`; the custom-function form is not yet supported over FFI.
   */
  signatureVerifier?: SignatureVerifierOptions;
}

/** Serialize options for the FFI: binary key data is base64-encoded so it survives the JSON wire. */
function serializeOptions(options: UpdaterOptions): string {
  const { signatureVerifier, ...rest } = options;
  if (signatureVerifier == null) {
    return JSON.stringify(rest);
  }
  const { key } = signatureVerifier;
  // PEM formats carry text; binary formats (Uint8Array) are base64-encoded for the JSON wire.
  const data = typeof key.data === 'string' ? key.data : encodeBase64(key.data);
  return JSON.stringify({
    ...rest,
    signatureVerifier: {
      algorithm: signatureVerifier.algorithm,
      key: { format: key.format, data },
    },
  });
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
 * Supports `channel`, `integrityPolicy`, and a declarative `signatureVerifier`. The custom-function
 * callback options of `@wvb/node` (`integrityChecker`, custom `signatureVerifier`) are not yet
 * supported over FFI.
 */
export class Updater {
  #ptr: Deno.PointerValue;

  constructor(source: BundleSource, remote: Remote, options?: UpdaterOptions) {
    const lib = getLib();
    this.#ptr = lib.symbols.wvb_updater_new(
      source.pointer,
      remote.pointer,
      cstr(options != null ? serializeOptions(options) : '')
    );
    if (this.#ptr === null) {
      // A null updater also means a provided `signatureVerifier` couldn't be built (fail closed).
      throw new Error('wvb: failed to create Updater (check signatureVerifier algorithm/key)');
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
