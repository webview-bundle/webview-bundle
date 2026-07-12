// The single source of truth for every error code this binding exposes. Included by `src/error.rs`
// (→ the `ErrorCode` enum) and by `build.rs` (→ `lib/error-codes.ts`), each expanding it with its
// own `error_codes!`. Add a code here and both sides follow.
error_codes! {
  CoreIo => "core.io",
  CoreCompress => "core.compress",
  CoreDecompress => "core.decompress",
  CoreEncode => "core.encode",
  CoreDecode => "core.decode",
  CoreHttp => "core.http",
  CoreInvalidMagicNum => "core.invalid_magic_num",
  CoreInvalidVersion => "core.invalid_version",
  CoreInvalidHeaderChecksum => "core.invalid_header_checksum",
  CoreInvalidIndexChecksum => "core.invalid_index_checksum",
  CoreChecksumMismatch => "core.checksum_mismatch",
  CoreBundleNotFound => "core.bundle_not_found",
  CoreBundleEntryNotExists => "core.bundle_entry_not_exists",
  CoreBundleCannotBeRemoved => "core.bundle_cannot_be_removed",
  CoreInvalidFilepath => "core.invalid_filepath",
  CoreSerdeJson => "core.serde_json",
  CoreCannotResolveProxyServer => "core.cannot_resolve_proxy_server",
  CoreReqwest => "core.reqwest",
  CoreInvalidRemoteUrl => "core.invalid_remote_url",
  CoreInvalidRemoteBundle => "core.invalid_remote_bundle",
  CoreRemoteBundleNotFound => "core.remote_bundle_not_found",
  CoreRemoteForbidden => "core.remote_forbidden",
  CoreRemoteHttp => "core.remote_http",
  CoreInvalidRemoteConfig => "core.invalid_remote_config",
  CoreInvalidIntegrity => "core.invalid_integrity",
  CoreIntegrityRequired => "core.integrity_required",
  CoreIntegrityVerifyFailed => "core.integrity_verify_failed",
  CoreInvalidSignature => "core.invalid_signature",
  CoreInvalidSigningKey => "core.invalid_signing_key",
  CoreSignatureSignFailed => "core.signature_sign_failed",
  CoreInvalidVerifyingKey => "core.invalid_verifying_key",
  CoreSignatureNotExists => "core.signature_not_exists",
  CoreSignatureVerifyFailed => "core.signature_verify_failed",
  CoreGeneric => "core.generic",
  InvalidMethod => "invalid_method",
  InvalidRequest => "invalid_request",
  /// Raised in `lib/updater.ts`: the native `Updater` constructor rejected `signatureVerifier`.
  #[allow(dead_code, reason = "TypeScript-side code; declared here to reach `ErrorCode`")]
  InvalidSignatureOptions => "invalid_signature_options",
  NullHandle => "null_handle",
  /// Raised in `lib/*.ts`: a handle constructor returned null, or an error payload carried no code.
  #[allow(dead_code, reason = "TypeScript-side code; declared here to reach `ErrorCode`")]
  Unknown => "unknown",
}
