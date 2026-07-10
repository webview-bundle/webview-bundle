//! Top-level error type exposed across the FFI boundary.
//!
//! The variants mirror [`wvb::ErrorCode`] one-for-one, so a failure is identified by the same
//! category here.
//!
//! The Rust `CoreIo` variant is the Kotlin
//! `WebviewBundleException.CoreIo` / Swift `WebviewBundleError.CoreIo`, and corresponds to the
//! `core.io` code the JavaScript bindings expose.

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
  /// The wire code for this error, matching the JavaScript bindings (e.g. `core.io`).
  pub fn code(&self) -> &'static str {
    match self {
      Self::CoreIo(_) => ErrorCode::Io.as_str(),
      Self::CoreCompress(_) => ErrorCode::Compress.as_str(),
      Self::CoreDecompress(_) => ErrorCode::Decompress.as_str(),
      Self::CoreEncode(_) => ErrorCode::Encode.as_str(),
      Self::CoreDecode(_) => ErrorCode::Decode.as_str(),
      Self::CoreHttp(_) => ErrorCode::Http.as_str(),
      Self::CoreInvalidMagicNum(_) => ErrorCode::InvalidMagicNum.as_str(),
      Self::CoreInvalidVersion(_) => ErrorCode::InvalidVersion.as_str(),
      Self::CoreInvalidHeaderChecksum(_) => ErrorCode::InvalidHeaderChecksum.as_str(),
      Self::CoreInvalidIndexChecksum(_) => ErrorCode::InvalidIndexChecksum.as_str(),
      Self::CoreChecksumMismatch(_) => ErrorCode::ChecksumMismatch.as_str(),
      Self::CoreBundleNotFound(_) => ErrorCode::BundleNotFound.as_str(),
      Self::CoreBundleEntryNotExists(_) => ErrorCode::BundleEntryNotExists.as_str(),
      Self::CoreBundleCannotBeRemoved(_) => ErrorCode::BundleCannotBeRemoved.as_str(),
      Self::CoreInvalidFilepath(_) => ErrorCode::InvalidFilepath.as_str(),
      Self::CoreSerdeJson(_) => ErrorCode::SerdeJson.as_str(),
      Self::CoreCannotResolveLocalHost(_) => ErrorCode::CannotResolveLocalHost.as_str(),
      Self::CoreReqwest(_) => ErrorCode::Reqwest.as_str(),
      Self::CoreInvalidRemoteUrl(_) => ErrorCode::InvalidRemoteUrl.as_str(),
      Self::CoreInvalidRemoteBundle(_) => ErrorCode::InvalidRemoteBundle.as_str(),
      Self::CoreRemoteBundleNotFound(_) => ErrorCode::RemoteBundleNotFound.as_str(),
      Self::CoreRemoteForbidden(_) => ErrorCode::RemoteForbidden.as_str(),
      Self::CoreRemoteHttp(_) => ErrorCode::RemoteHttp.as_str(),
      Self::CoreInvalidRemoteConfig(_) => ErrorCode::InvalidRemoteConfig.as_str(),
      Self::CoreInvalidIntegrity(_) => ErrorCode::InvalidIntegrity.as_str(),
      Self::CoreIntegrityRequired(_) => ErrorCode::IntegrityRequired.as_str(),
      Self::CoreIntegrityVerifyFailed(_) => ErrorCode::IntegrityVerifyFailed.as_str(),
      Self::CoreInvalidSignature(_) => ErrorCode::InvalidSignature.as_str(),
      Self::CoreInvalidSigningKey(_) => ErrorCode::InvalidSigningKey.as_str(),
      Self::CoreSignatureSignFailed(_) => ErrorCode::SignatureSignFailed.as_str(),
      Self::CoreInvalidVerifyingKey(_) => ErrorCode::InvalidVerifyingKey.as_str(),
      Self::CoreSignatureNotExists(_) => ErrorCode::SignatureNotExists.as_str(),
      Self::CoreSignatureVerifyFailed(_) => ErrorCode::SignatureVerifyFailed.as_str(),
      Self::CoreGeneric(_) => ErrorCode::Generic.as_str(),
      Self::BindingInvalidHeaderName(_) => "binding.invalid_header_name",
      Self::BindingInvalidHeaderValue(_) => "binding.invalid_header_value",
      Self::BindingInvalidSignatureOptions(_) => "binding.invalid_signature_options",
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
