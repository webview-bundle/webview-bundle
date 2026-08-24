import { encodeBase64 } from '@std/encoding/base64';
import type { SignatureAlgorithm, SignatureKeyFormat } from './bindings.ts';
import { WebviewBundleError } from './error.ts';

export type { SignatureAlgorithm, SignatureKeyFormat };

/** The public key itself, in one of the formats the algorithm accepts. */
export interface SignatureKeyData {
  format: SignatureKeyFormat;
  /** PEM text for `'spki_pem'`/`'pkcs1_pem'`; the key bytes for every other format. */
  data: string | Uint8Array;
}

export interface SignatureKey {
  algorithm: SignatureAlgorithm;
  key: SignatureKeyData;
}

/**
 * A verifying key paired with the id it is published under, so an update naming a `keyId` can be
 * matched to the key that verifies it.
 *
 * Unlike `@wvb/node`, `verify` cannot be a JavaScript callback: a `nonblocking` FFI call runs on a
 * worker thread that cannot re-enter the JS event loop, so a custom verifier would deadlock.
 */
export interface SignatureVerifyKey {
  id: string;
  verify: SignatureKey;
}

const PEM_FORMATS: ReadonlySet<SignatureKeyFormat> = new Set(['spki_pem', 'pkcs1_pem']);

/**
 * @internal Encode a key for the JSON wire: PEM formats carry their text as-is, and the binary
 * formats carry base64. A `data` whose type does not match its format is rejected here rather than
 * reaching the native side as an unparsable key.
 */
export function serializeSignatureVerifyKey(key: SignatureVerifyKey): SignatureVerifyKey {
  const { format, data } = key.verify.key;
  if (PEM_FORMATS.has(format)) {
    if (typeof data !== 'string') {
      throw new WebviewBundleError(
        'invalid_signature_key',
        `wvb: signature key must be a string for the '${format}' format`
      );
    }
    return key;
  }
  if (typeof data === 'string') {
    throw new WebviewBundleError(
      'invalid_signature_key',
      `wvb: signature key must be a Uint8Array for the '${format}' format`
    );
  }
  return {
    id: key.id,
    verify: { algorithm: key.verify.algorithm, key: { format, data: encodeBase64(data) } },
  };
}
