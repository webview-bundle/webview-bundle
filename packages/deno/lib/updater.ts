import { encodeBase64 } from '@std/encoding/base64';
import { WebviewBundleError } from './error.ts';
import { cstr, getLib, readResult } from './ffi.ts';
import type { ListRemoteBundleInfo, Remote, RemoteBundleInfo } from './remote.ts';
import type { BundleSource } from './source.ts';

export type IntegrityPolicy = 'strict' | 'optional' | 'none';

export type SignatureAlgorithm =
  | 'ecdsaSecp256R1'
  | 'ecdsaSecp384R1'
  | 'ed25519'
  | 'rsaPkcs1V15'
  | 'rsaPss';

export type VerifyingKeyFormat = 'spkiDer' | 'spkiPem' | 'pkcs1Der' | 'pkcs1Pem' | 'sec1' | 'raw';

export interface SignatureVerifyingKeyOptions {
  format: VerifyingKeyFormat;
  data: string | Uint8Array;
}

export interface SignatureVerifierOptions {
  algorithm: SignatureAlgorithm;
  key: SignatureVerifyingKeyOptions;
}

export interface UpdaterOptions {
  channel?: string;
  integrityPolicy?: IntegrityPolicy;
  /**
   * Verify bundle signatures against a public key.
   */
  signatureVerifier?: SignatureVerifierOptions;
}

/**
 * @internal Encodes a verifier for the JSON wire, shared with {@link BundleSource} so both
 * sides of the FFI agree on one encoding.
 */
export function serializeSignatureVerifier(
  verifier: SignatureVerifierOptions
): SignatureVerifierOptions {
  const { key } = verifier;
  // PEM formats carry text; binary formats (Uint8Array) are base64-encoded for the JSON wire.
  const data = typeof key.data === 'string' ? key.data : encodeBase64(key.data);
  return {
    algorithm: verifier.algorithm,
    key: { format: key.format, data },
  };
}

function serializeOptions(options: UpdaterOptions): string {
  const { signatureVerifier, ...rest } = options;
  if (signatureVerifier == null) {
    return JSON.stringify(rest);
  }
  return JSON.stringify({
    ...rest,
    signatureVerifier: serializeSignatureVerifier(signatureVerifier),
  });
}

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
      throw new WebviewBundleError(
        'invalid_signature_options',
        'wvb: failed to create Updater (check signatureVerifier algorithm/key)'
      );
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
