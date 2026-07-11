//! Top-level error type exposed across the FFI boundary.
//!
//! The `Core*` variants mirror [`wvb::ErrorCode`] one-for-one, so a core failure is identified by
//! the same category here. The `Binding*` variants (invalid HTTP headers, malformed
//! signature-verifier options) originate in this FFI layer and have no `ErrorCode` counterpart.
//!
//! The Rust `CoreIo` variant is the Kotlin `WebviewBundleException.CoreIo` / Swift
//! `WebviewBundleError.CoreIo`, and corresponds to the `core.io` code the JavaScript bindings
//! expose.

use wvb::ErrorCode;

/// WebviewBundle Error.
#[derive(thiserror::Error, uniffi::Error, Debug)]
#[uniffi(flat_error, name = "WebviewBundleError")]
pub enum Error {
  #[error("{0}")]
  CoreIo(String),
  #[error("{0}")]
  CoreCompress(String),
  #[error("{0}")]
  CoreDecompress(String),
  #[error("{0}")]
  CoreEncode(String),
  #[error("{0}")]
  CoreDecode(String),
  #[error("{0}")]
  CoreHttp(String),
  #[error("{0}")]
  CoreInvalidMagicNum(String),
  #[error("{0}")]
  CoreInvalidVersion(String),
  #[error("{0}")]
  CoreInvalidHeaderChecksum(String),
  #[error("{0}")]
  CoreInvalidIndexChecksum(String),
  #[error("{0}")]
  CoreChecksumMismatch(String),
  #[error("{0}")]
  CoreBundleNotFound(String),
  #[error("{0}")]
  CoreBundleEntryNotExists(String),
  #[error("{0}")]
  CoreBundleCannotBeRemoved(String),
  #[error("{0}")]
  CoreInvalidFilepath(String),
  #[error("{0}")]
  CoreSerdeJson(String),
  #[error("{0}")]
  CoreCannotResolveLocalHost(String),
  #[error("{0}")]
  CoreReqwest(String),
  #[error("{0}")]
  CoreInvalidRemoteUrl(String),
  #[error("{0}")]
  CoreInvalidRemoteBundle(String),
  #[error("{0}")]
  CoreRemoteBundleNotFound(String),
  #[error("{0}")]
  CoreRemoteForbidden(String),
  #[error("{0}")]
  CoreRemoteHttp(String),
  #[error("{0}")]
  CoreInvalidRemoteConfig(String),
  #[error("{0}")]
  CoreInvalidIntegrity(String),
  #[error("{0}")]
  CoreIntegrityRequired(String),
  #[error("{0}")]
  CoreIntegrityVerifyFailed(String),
  #[error("{0}")]
  CoreInvalidSignature(String),
  #[error("{0}")]
  CoreInvalidSigningKey(String),
  #[error("{0}")]
  CoreSignatureSignFailed(String),
  #[error("{0}")]
  CoreInvalidVerifyingKey(String),
  #[error("{0}")]
  CoreSignatureNotExists(String),
  #[error("{0}")]
  CoreSignatureVerifyFailed(String),
  #[error("{0}")]
  CoreGeneric(String),
  /// Invalid HTTP header name.
  #[error("{0}")]
  BindingInvalidHeaderName(String),
  /// Invalid HTTP header value.
  #[error("{0}")]
  BindingInvalidHeaderValue(String),
  /// The `SignatureVerifierOptions` passed across the boundary could not be turned into a
  /// verifier (unsupported algorithm/format pairing, or a key of the wrong shape).
  #[error("{0}")]
  BindingInvalidSignatureOptions(String),
}

impl Error {
  /// The wire code for this error, matching the JavaScript bindings: core failures are namespaced
  /// under `core.` (e.g. `core.io`); binding-local failures are unprefixed (e.g.
  /// `invalid_header_name`).
  pub fn code(&self) -> &'static str {
    match self {
      Self::CoreIo(_) => "core.io",
      Self::CoreCompress(_) => "core.compress",
      Self::CoreDecompress(_) => "core.decompress",
      Self::CoreEncode(_) => "core.encode",
      Self::CoreDecode(_) => "core.decode",
      Self::CoreHttp(_) => "core.http",
      Self::CoreInvalidMagicNum(_) => "core.invalid_magic_num",
      Self::CoreInvalidVersion(_) => "core.invalid_version",
      Self::CoreInvalidHeaderChecksum(_) => "core.invalid_header_checksum",
      Self::CoreInvalidIndexChecksum(_) => "core.invalid_index_checksum",
      Self::CoreChecksumMismatch(_) => "core.checksum_mismatch",
      Self::CoreBundleNotFound(_) => "core.bundle_not_found",
      Self::CoreBundleEntryNotExists(_) => "core.bundle_entry_not_exists",
      Self::CoreBundleCannotBeRemoved(_) => "core.bundle_cannot_be_removed",
      Self::CoreInvalidFilepath(_) => "core.invalid_filepath",
      Self::CoreSerdeJson(_) => "core.serde_json",
      Self::CoreCannotResolveLocalHost(_) => "core.cannot_resolve_local_host",
      Self::CoreReqwest(_) => "core.reqwest",
      Self::CoreInvalidRemoteUrl(_) => "core.invalid_remote_url",
      Self::CoreInvalidRemoteBundle(_) => "core.invalid_remote_bundle",
      Self::CoreRemoteBundleNotFound(_) => "core.remote_bundle_not_found",
      Self::CoreRemoteForbidden(_) => "core.remote_forbidden",
      Self::CoreRemoteHttp(_) => "core.remote_http",
      Self::CoreInvalidRemoteConfig(_) => "core.invalid_remote_config",
      Self::CoreInvalidIntegrity(_) => "core.invalid_integrity",
      Self::CoreIntegrityRequired(_) => "core.integrity_required",
      Self::CoreIntegrityVerifyFailed(_) => "core.integrity_verify_failed",
      Self::CoreInvalidSignature(_) => "core.invalid_signature",
      Self::CoreInvalidSigningKey(_) => "core.invalid_signing_key",
      Self::CoreSignatureSignFailed(_) => "core.signature_sign_failed",
      Self::CoreInvalidVerifyingKey(_) => "core.invalid_verifying_key",
      Self::CoreSignatureNotExists(_) => "core.signature_not_exists",
      Self::CoreSignatureVerifyFailed(_) => "core.signature_verify_failed",
      Self::CoreGeneric(_) => "core.generic",
      Self::BindingInvalidHeaderName(_) => "invalid_header_name",
      Self::BindingInvalidHeaderValue(_) => "invalid_header_value",
      Self::BindingInvalidSignatureOptions(_) => "invalid_signature_options",
    }
  }

  pub(crate) fn invalid_signature_options(message: impl Into<String>) -> Self {
    Self::BindingInvalidSignatureOptions(message.into())
  }
}

impl From<wvb::Error> for Error {
  fn from(e: wvb::Error) -> Self {
    let message = e.to_string();
    match e.code() {
      ErrorCode::Io => Self::CoreIo(message),
      ErrorCode::Compress => Self::CoreCompress(message),
      ErrorCode::Decompress => Self::CoreDecompress(message),
      ErrorCode::Encode => Self::CoreEncode(message),
      ErrorCode::Decode => Self::CoreDecode(message),
      ErrorCode::Http => Self::CoreHttp(message),
      ErrorCode::InvalidMagicNum => Self::CoreInvalidMagicNum(message),
      ErrorCode::InvalidVersion => Self::CoreInvalidVersion(message),
      ErrorCode::InvalidHeaderChecksum => Self::CoreInvalidHeaderChecksum(message),
      ErrorCode::InvalidIndexChecksum => Self::CoreInvalidIndexChecksum(message),
      ErrorCode::ChecksumMismatch => Self::CoreChecksumMismatch(message),
      ErrorCode::BundleNotFound => Self::CoreBundleNotFound(message),
      ErrorCode::BundleEntryNotExists => Self::CoreBundleEntryNotExists(message),
      ErrorCode::BundleCannotBeRemoved => Self::CoreBundleCannotBeRemoved(message),
      ErrorCode::InvalidFilepath => Self::CoreInvalidFilepath(message),
      ErrorCode::SerdeJson => Self::CoreSerdeJson(message),
      ErrorCode::CannotResolveLocalHost => Self::CoreCannotResolveLocalHost(message),
      ErrorCode::Reqwest => Self::CoreReqwest(message),
      ErrorCode::InvalidRemoteUrl => Self::CoreInvalidRemoteUrl(message),
      ErrorCode::InvalidRemoteBundle => Self::CoreInvalidRemoteBundle(message),
      ErrorCode::RemoteBundleNotFound => Self::CoreRemoteBundleNotFound(message),
      ErrorCode::RemoteForbidden => Self::CoreRemoteForbidden(message),
      ErrorCode::RemoteHttp => Self::CoreRemoteHttp(message),
      ErrorCode::InvalidRemoteConfig => Self::CoreInvalidRemoteConfig(message),
      ErrorCode::InvalidIntegrity => Self::CoreInvalidIntegrity(message),
      ErrorCode::IntegrityRequired => Self::CoreIntegrityRequired(message),
      ErrorCode::IntegrityVerifyFailed => Self::CoreIntegrityVerifyFailed(message),
      ErrorCode::InvalidSignature => Self::CoreInvalidSignature(message),
      ErrorCode::InvalidSigningKey => Self::CoreInvalidSigningKey(message),
      ErrorCode::SignatureSignFailed => Self::CoreSignatureSignFailed(message),
      ErrorCode::InvalidVerifyingKey => Self::CoreInvalidVerifyingKey(message),
      ErrorCode::SignatureNotExists => Self::CoreSignatureNotExists(message),
      ErrorCode::SignatureVerifyFailed => Self::CoreSignatureVerifyFailed(message),
      ErrorCode::Generic => Self::CoreGeneric(message),
    }
  }
}

impl From<http::header::InvalidHeaderName> for Error {
  fn from(e: http::header::InvalidHeaderName) -> Self {
    Self::BindingInvalidHeaderName(e.to_string())
  }
}

impl From<http::header::InvalidHeaderValue> for Error {
  fn from(e: http::header::InvalidHeaderValue) -> Self {
    Self::BindingInvalidHeaderValue(e.to_string())
  }
}
