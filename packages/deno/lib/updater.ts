import { encodeBase64 } from '@std/encoding/base64';
import type {
  BundleUpdateInfo,
  IntegrityPolicy,
  SignatureAlgorithm,
  VerifyingKeyFormat,
} from './bindings.ts';
import { WebviewBundleError } from './error.ts';
import { cstr, getLib, readResult } from './ffi.ts';
import type { ListRemoteBundleInfo, Remote, RemoteBundleInfo } from './remote.ts';
import type { BundleSource } from './source.ts';

export type { BundleUpdateInfo, IntegrityPolicy, SignatureAlgorithm, VerifyingKeyFormat };

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
   * Verify that a downloaded bundle's integrity string was signed by the matching key.
   *
   * The signature signs the bundle's integrity string, not the bundle bytes, so verifying it
   * proves the integrity string is authentic — not that the downloaded bytes match it. It is
   * verified independently of {@link UpdaterOptions.integrityPolicy}, so keep the policy enabled
   * (not `'off'`) for the signature to also authenticate the downloaded bytes.
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
      // A null updater means an option was ill-formed — an `integrityPolicy` value the native
      // side rejected, or a `signatureVerifier` it couldn't build. Fail closed rather than serve
      // updates unverified; only blame the key when one was actually given.
      throw options?.signatureVerifier != null
        ? new WebviewBundleError(
            'invalid_signature_options',
            'wvb: failed to create Updater (check signatureVerifier algorithm/key)'
          )
        : new WebviewBundleError(
            'unknown',
            'wvb: failed to create Updater (check integrityPolicy)'
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
