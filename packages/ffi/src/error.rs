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
  CoreHttpInvalidUri(String),
  #[error("{0}")]
  CoreCancelled(String),
  #[error("{0}")]
  CoreTimeout(String),
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
  CoreInvalidFilepath(String),
  #[error("{0}")]
  CoreSerdeJson(String),
  #[error("{0}")]
  CoreCannotResolveProxyServer(String),
  #[error("{0}")]
  CoreHttpClient(String),
  #[error("{0}")]
  CoreRemoteHttp(String),
  #[error("{0}")]
  CoreBadRemoteResponse(String),
  #[error("{0}")]
  CoreInvalidRemoteConfig(String),
  #[error("{0}")]
  CoreInvalidUpdaterConfig(String),
  #[error("{0}")]
  CoreInvalidIntegrity(String),
  #[error("{0}")]
  CoreIntegrityRequired(String),
  #[error("{0}")]
  CoreIntegrityVerifyFailed(String),
  #[error("{0}")]
  CoreInvalidSignature(String),
  #[error("{0}")]
  CoreInvalidSignatureKey(String),
  #[error("{0}")]
  CoreExpectSignatureNotFound(String),
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
  /// The signature verify options passed across the boundary could not be turned into a
  /// verifier (a key of the wrong shape, or one the algorithm cannot read).
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
      Self::CoreHttpInvalidUri(_) => "core.http_invalid_uri",
      Self::CoreCancelled(_) => "core.cancelled",
      Self::CoreTimeout(_) => "core.timeout",
      Self::CoreInvalidMagicNum(_) => "core.invalid_magic_num",
      Self::CoreInvalidVersion(_) => "core.invalid_version",
      Self::CoreInvalidHeaderChecksum(_) => "core.invalid_header_checksum",
      Self::CoreInvalidIndexChecksum(_) => "core.invalid_index_checksum",
      Self::CoreChecksumMismatch(_) => "core.checksum_mismatch",
      Self::CoreBundleNotFound(_) => "core.bundle_not_found",
      Self::CoreInvalidFilepath(_) => "core.invalid_filepath",
      Self::CoreSerdeJson(_) => "core.serde_json",
      Self::CoreCannotResolveProxyServer(_) => "core.cannot_resolve_proxy_server",
      Self::CoreHttpClient(_) => "core.http_client",
      Self::CoreRemoteHttp(_) => "core.remote_http",
      Self::CoreBadRemoteResponse(_) => "core.bad_remote_response",
      Self::CoreInvalidRemoteConfig(_) => "core.invalid_remote_config",
      Self::CoreInvalidUpdaterConfig(_) => "core.invalid_updater_config",
      Self::CoreInvalidIntegrity(_) => "core.invalid_integrity",
      Self::CoreIntegrityRequired(_) => "core.integrity_required",
      Self::CoreIntegrityVerifyFailed(_) => "core.integrity_verify_failed",
      Self::CoreInvalidSignature(_) => "core.invalid_signature",
      Self::CoreInvalidSignatureKey(_) => "core.invalid_signature_key",
      Self::CoreExpectSignatureNotFound(_) => "core.expect_signature_not_found",
      Self::CoreSignatureVerifyFailed(_) => "core.signature_verify_failed",
      Self::CoreGeneric(_) => "core.generic",
      Self::BindingInvalidHeaderName(_) => "invalid_header_name",
      Self::BindingInvalidHeaderValue(_) => "invalid_header_value",
      Self::BindingInvalidSignatureOptions(_) => "invalid_signature_options",
    }
  }

  pub(crate) fn invalid_signature_verify(message: impl Into<String>) -> Self {
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
      ErrorCode::HttpInvalidUri => Self::CoreHttpInvalidUri(message),
      ErrorCode::Cancelled => Self::CoreCancelled(message),
      ErrorCode::Timeout => Self::CoreTimeout(message),
      ErrorCode::InvalidMagicNum => Self::CoreInvalidMagicNum(message),
      ErrorCode::InvalidVersion => Self::CoreInvalidVersion(message),
      ErrorCode::InvalidHeaderChecksum => Self::CoreInvalidHeaderChecksum(message),
      ErrorCode::InvalidIndexChecksum => Self::CoreInvalidIndexChecksum(message),
      ErrorCode::ChecksumMismatch => Self::CoreChecksumMismatch(message),
      ErrorCode::BundleNotFound => Self::CoreBundleNotFound(message),
      ErrorCode::InvalidFilepath => Self::CoreInvalidFilepath(message),
      ErrorCode::SerdeJson => Self::CoreSerdeJson(message),
      ErrorCode::CannotResolveProxyServer => Self::CoreCannotResolveProxyServer(message),
      ErrorCode::HttpClient => Self::CoreHttpClient(message),
      ErrorCode::RemoteHttp => Self::CoreRemoteHttp(message),
      ErrorCode::BadRemoteResponse => Self::CoreBadRemoteResponse(message),
      ErrorCode::InvalidRemoteConfig => Self::CoreInvalidRemoteConfig(message),
      ErrorCode::InvalidUpdaterConfig => Self::CoreInvalidUpdaterConfig(message),
      ErrorCode::InvalidIntegrity => Self::CoreInvalidIntegrity(message),
      ErrorCode::IntegrityRequired => Self::CoreIntegrityRequired(message),
      ErrorCode::IntegrityVerifyFailed => Self::CoreIntegrityVerifyFailed(message),
      ErrorCode::InvalidSignature => Self::CoreInvalidSignature(message),
      ErrorCode::InvalidSignatureKey => Self::CoreInvalidSignatureKey(message),
      ErrorCode::ExpectSignatureNotFound => Self::CoreExpectSignatureNotFound(message),
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
