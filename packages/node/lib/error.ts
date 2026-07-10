// The error type shared by every webview-bundle binding.
//
// `code` is the stable identifier for a failure and is identical across `@wvb/node`, `@wvb/deno`,
// and the Kotlin/Swift FFI (where `core.bundle_not_found` is `WebviewBundleException.CoreBundleNotFound`).
// `core.*` codes come from the `wvb` Rust core; unprefixed codes (e.g. `napi`) are raised by this
// binding.

/** Every error code `@wvb/node` can raise, in declaration order. */
export const WEBVIEW_BUNDLE_ERROR_CODES = [
  'core.io',
  'core.compress',
  'core.decompress',
  'core.encode',
  'core.decode',
  'core.http',
  'core.invalid_magic_num',
  'core.invalid_version',
  'core.invalid_header_checksum',
  'core.invalid_index_checksum',
  'core.checksum_mismatch',
  'core.bundle_not_found',
  'core.bundle_entry_not_exists',
  'core.bundle_cannot_be_removed',
  'core.invalid_filepath',
  'core.serde_json',
  'core.cannot_resolve_local_host',
  'core.reqwest',
  'core.invalid_remote_url',
  'core.invalid_remote_bundle',
  'core.remote_bundle_not_found',
  'core.remote_forbidden',
  'core.remote_http',
  'core.invalid_remote_config',
  'core.invalid_integrity',
  'core.integrity_required',
  'core.integrity_verify_failed',
  'core.invalid_signature',
  'core.invalid_signing_key',
  'core.signature_sign_failed',
  'core.invalid_verifying_key',
  'core.signature_not_exists',
  'core.signature_verify_failed',
  'core.generic',
  /** An HTTP header name rejected by the binding. */
  'invalid_header_name',
  /** An HTTP header value rejected by the binding. */
  'invalid_header_value',
  /** `signatureVerifier` options that could not be turned into a verifier. */
  'invalid_signature_options',
  /** A native handle was null, or used after being freed. */
  'null_handle',
  'napi',
  /** The binding could not classify the failure. */
  'unknown',
] as const;

export type WebviewBundleErrorCode = (typeof WEBVIEW_BUNDLE_ERROR_CODES)[number];

const KNOWN_CODES: ReadonlySet<string> = new Set(WEBVIEW_BUNDLE_ERROR_CODES);

export class WebviewBundleError extends Error {
  override readonly name = 'WebviewBundleError';
  readonly code: WebviewBundleErrorCode;

  constructor(code: WebviewBundleErrorCode, message: string, options?: ErrorOptions) {
    super(message, options);
    this.code = code;
  }
}

export function isWebviewBundleError(value: unknown): value is WebviewBundleError {
  return (
    value != null &&
    typeof value === 'object' &&
    'name' in value &&
    value.name === 'WebviewBundleError'
  );
}

export function isWebviewBundleErrorCode(value: unknown): value is WebviewBundleErrorCode {
  return typeof value === 'string' && KNOWN_CODES.has(value);
}

// `src/error.rs` prefixes the message it throws with `[code=<code>] ` (napi cannot attach a real
// `code` property to a rejected promise). This matches that prefix; keep it in sync with `tagged()`
// there. The code may contain dots (`core.io`).
const TAGGED_MESSAGE = /^\[code=([a-z0-9_.]+)] ([\s\S]*)$/;

/**
 * Convert a native error into a {@link WebviewBundleError}.
 *
 * Every error reaching this function came across the native boundary, so it is classified:
 * - a `[code=…]`-tagged error → that code (or `unknown` if this JS layer is older than the binary);
 * - any other error → `napi`. These are napi's own failures — a wrong argument type (thrown by
 *   napi's generated glue, which bypasses our Rust `From<Error>`), an exception thrown by a JS
 *   callback, or a host/threadsafe-function error. The original is kept as `cause`.
 */
export function toWebviewBundleError(value: unknown): unknown {
  if (isWebviewBundleError(value) || !(value instanceof Error)) {
    return value;
  }
  const matched = TAGGED_MESSAGE.exec(value.message);
  if (matched == null) {
    return new WebviewBundleError('napi', value.message, { cause: value });
  }
  const [, code = '', message = ''] = matched;
  const known = isWebviewBundleErrorCode(code) ? code : 'unknown';
  return new WebviewBundleError(known, message, { cause: value });
}
