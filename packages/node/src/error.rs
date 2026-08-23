use napi_derive::napi;
use wvb::http;

/// Declares [`ErrorCode`] once: the string enum TypeScript sees, and the wire code each variant is
/// tagged with.
macro_rules! error_codes {
  ($($(#[$attr:meta])* $variant:ident => $value:literal),+ $(,)?) => {
    /// The stable code every error thrown by this binding carries (`WebviewBundleError.code`).
    #[napi(string_enum)]
    pub enum ErrorCode {
      $(
        $(#[$attr])*
        #[napi(value = $value)]
        $variant,
      )+
    }

    impl ErrorCode {
      pub(crate) fn as_str(&self) -> &'static str {
        match self {
          $(Self::$variant => $value,)+
        }
      }
    }
  };
}

error_codes! {
  CoreIo => "core.io",
  CoreCompress => "core.compress",
  CoreDecompress => "core.decompress",
  CoreEncode => "core.encode",
  CoreDecode => "core.decode",
  CoreHttp => "core.http",
  CoreHttpInvalidUri => "core.http_invalid_uri",
  CoreCancelled => "core.cancelled",
  CoreTimeout => "core.timeout",
  CoreInvalidMagicNum => "core.invalid_magic_num",
  CoreInvalidVersion => "core.invalid_version",
  CoreInvalidHeaderChecksum => "core.invalid_header_checksum",
  CoreInvalidIndexChecksum => "core.invalid_index_checksum",
  CoreChecksumMismatch => "core.checksum_mismatch",
  CoreBundleNotFound => "core.bundle_not_found",
  CoreInvalidFilepath => "core.invalid_filepath",
  CoreSerdeJson => "core.serde_json",
  CoreCannotResolveProxyServer => "core.cannot_resolve_proxy_server",
  CoreHttpClient => "core.http_client",
  CoreRemoteHttp => "core.remote_http",
  CoreBadRemoteResponse => "core.bad_remote_response",
  CoreInvalidRemoteConfig => "core.invalid_remote_config",
  CoreInvalidUpdaterConfig => "core.invalid_updater_config",
  CoreInvalidIntegrity => "core.invalid_integrity",
  CoreIntegrityRequired => "core.integrity_required",
  CoreIntegrityVerifyFailed => "core.integrity_verify_failed",
  CoreInvalidSignature => "core.invalid_signature",
  CoreInvalidSignatureKey => "core.invalid_signature_key",
  CoreExpectSignatureNotFound => "core.expect_signature_not_found",
  CoreSignatureVerifyFailed => "core.signature_verify_failed",
  CoreGeneric => "core.generic",
  InvalidHeaderName => "invalid_header_name",
  InvalidHeaderValue => "invalid_header_value",
  InvalidSignatureKey => "invalid_signature_key",
  Napi => "napi",
}

/// Exhaustive: a code added to the core fails to compile here until it is exposed to TypeScript.
impl From<wvb::ErrorCode> for ErrorCode {
  fn from(code: wvb::ErrorCode) -> Self {
    match code {
      wvb::ErrorCode::Io => Self::CoreIo,
      wvb::ErrorCode::Compress => Self::CoreCompress,
      wvb::ErrorCode::Decompress => Self::CoreDecompress,
      wvb::ErrorCode::Encode => Self::CoreEncode,
      wvb::ErrorCode::Decode => Self::CoreDecode,
      wvb::ErrorCode::Http => Self::CoreHttp,
      wvb::ErrorCode::HttpInvalidUri => Self::CoreHttpInvalidUri,
      wvb::ErrorCode::Cancelled => Self::CoreCancelled,
      wvb::ErrorCode::Timeout => Self::CoreTimeout,
      wvb::ErrorCode::InvalidMagicNum => Self::CoreInvalidMagicNum,
      wvb::ErrorCode::InvalidVersion => Self::CoreInvalidVersion,
      wvb::ErrorCode::InvalidHeaderChecksum => Self::CoreInvalidHeaderChecksum,
      wvb::ErrorCode::InvalidIndexChecksum => Self::CoreInvalidIndexChecksum,
      wvb::ErrorCode::ChecksumMismatch => Self::CoreChecksumMismatch,
      wvb::ErrorCode::BundleNotFound => Self::CoreBundleNotFound,
      wvb::ErrorCode::InvalidFilepath => Self::CoreInvalidFilepath,
      wvb::ErrorCode::SerdeJson => Self::CoreSerdeJson,
      wvb::ErrorCode::CannotResolveProxyServer => Self::CoreCannotResolveProxyServer,
      wvb::ErrorCode::HttpClient => Self::CoreHttpClient,
      wvb::ErrorCode::RemoteHttp => Self::CoreRemoteHttp,
      wvb::ErrorCode::BadRemoteResponse => Self::CoreBadRemoteResponse,
      wvb::ErrorCode::InvalidRemoteConfig => Self::CoreInvalidRemoteConfig,
      wvb::ErrorCode::InvalidUpdaterConfig => Self::CoreInvalidUpdaterConfig,
      wvb::ErrorCode::InvalidIntegrity => Self::CoreInvalidIntegrity,
      wvb::ErrorCode::IntegrityRequired => Self::CoreIntegrityRequired,
      wvb::ErrorCode::IntegrityVerifyFailed => Self::CoreIntegrityVerifyFailed,
      wvb::ErrorCode::InvalidSignature => Self::CoreInvalidSignature,
      wvb::ErrorCode::InvalidSignatureKey => Self::CoreInvalidSignatureKey,
      wvb::ErrorCode::ExpectSignatureNotFound => Self::CoreExpectSignatureNotFound,
      wvb::ErrorCode::SignatureVerifyFailed => Self::CoreSignatureVerifyFailed,
      wvb::ErrorCode::Generic => Self::CoreGeneric,
    }
  }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
  #[error(transparent)]
  Core(#[from] wvb::Error),
  #[error(transparent)]
  InvalidHeaderName(#[from] http::header::InvalidHeaderName),
  #[error(transparent)]
  InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),
  #[error("{0}")]
  InvalidSignatureKey(String),
  #[error(transparent)]
  Napi(#[from] napi::Error),
}

impl Error {
  pub(crate) fn invalid_signature_key(message: impl Into<String>) -> Self {
    Self::InvalidSignatureKey(message.into())
  }

  pub(crate) fn code(&self) -> ErrorCode {
    match self {
      Self::Core(e) => e.code().into(),
      Self::InvalidHeaderName(_) => ErrorCode::InvalidHeaderName,
      Self::InvalidHeaderValue(_) => ErrorCode::InvalidHeaderValue,
      Self::InvalidSignatureKey(_) => ErrorCode::InvalidSignatureKey,
      Self::Napi(_) => ErrorCode::Napi,
    }
  }
}

impl From<Error> for napi::Error {
  fn from(value: Error) -> Self {
    // `lib/error.ts` parses this prefix back into `WebviewBundleError.code`.
    let message = format!("[code={}] {value}", value.code().as_str());
    match value {
      Error::Core(_) => napi::Error::new(napi::Status::GenericFailure, message),
      Error::InvalidHeaderName(_)
      | Error::InvalidHeaderValue(_)
      | Error::InvalidSignatureKey(_) => napi::Error::new(napi::Status::InvalidArg, message),
      // Already a napi error: it keeps its own status and message.
      Error::Napi(e) => e,
    }
  }
}

impl From<Error> for napi::JsError {
  fn from(value: Error) -> Self {
    napi::JsError::from(napi::Error::from(value))
  }
}
