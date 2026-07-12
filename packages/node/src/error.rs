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
  InvalidHeaderName => "invalid_header_name",
  InvalidHeaderValue => "invalid_header_value",
  InvalidSignatureOptions => "invalid_signature_options",
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
      wvb::ErrorCode::InvalidMagicNum => Self::CoreInvalidMagicNum,
      wvb::ErrorCode::InvalidVersion => Self::CoreInvalidVersion,
      wvb::ErrorCode::InvalidHeaderChecksum => Self::CoreInvalidHeaderChecksum,
      wvb::ErrorCode::InvalidIndexChecksum => Self::CoreInvalidIndexChecksum,
      wvb::ErrorCode::ChecksumMismatch => Self::CoreChecksumMismatch,
      wvb::ErrorCode::BundleNotFound => Self::CoreBundleNotFound,
      wvb::ErrorCode::BundleEntryNotExists => Self::CoreBundleEntryNotExists,
      wvb::ErrorCode::BundleCannotBeRemoved => Self::CoreBundleCannotBeRemoved,
      wvb::ErrorCode::InvalidFilepath => Self::CoreInvalidFilepath,
      wvb::ErrorCode::SerdeJson => Self::CoreSerdeJson,
      wvb::ErrorCode::CannotResolveProxyServer => Self::CoreCannotResolveProxyServer,
      wvb::ErrorCode::Reqwest => Self::CoreReqwest,
      wvb::ErrorCode::InvalidRemoteUrl => Self::CoreInvalidRemoteUrl,
      wvb::ErrorCode::InvalidRemoteBundle => Self::CoreInvalidRemoteBundle,
      wvb::ErrorCode::RemoteBundleNotFound => Self::CoreRemoteBundleNotFound,
      wvb::ErrorCode::RemoteForbidden => Self::CoreRemoteForbidden,
      wvb::ErrorCode::RemoteHttp => Self::CoreRemoteHttp,
      wvb::ErrorCode::InvalidRemoteConfig => Self::CoreInvalidRemoteConfig,
      wvb::ErrorCode::InvalidIntegrity => Self::CoreInvalidIntegrity,
      wvb::ErrorCode::IntegrityRequired => Self::CoreIntegrityRequired,
      wvb::ErrorCode::IntegrityVerifyFailed => Self::CoreIntegrityVerifyFailed,
      wvb::ErrorCode::InvalidSignature => Self::CoreInvalidSignature,
      wvb::ErrorCode::InvalidSigningKey => Self::CoreInvalidSigningKey,
      wvb::ErrorCode::SignatureSignFailed => Self::CoreSignatureSignFailed,
      wvb::ErrorCode::InvalidVerifyingKey => Self::CoreInvalidVerifyingKey,
      wvb::ErrorCode::SignatureNotExists => Self::CoreSignatureNotExists,
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
  InvalidSignatureOptions(String),
  #[error(transparent)]
  Napi(#[from] napi::Error),
}

impl Error {
  pub(crate) fn invalid_signature_options(message: impl Into<String>) -> Self {
    Self::InvalidSignatureOptions(message.into())
  }

  pub(crate) fn code(&self) -> ErrorCode {
    match self {
      Self::Core(e) => e.code().into(),
      Self::InvalidHeaderName(_) => ErrorCode::InvalidHeaderName,
      Self::InvalidHeaderValue(_) => ErrorCode::InvalidHeaderValue,
      Self::InvalidSignatureOptions(_) => ErrorCode::InvalidSignatureOptions,
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
      | Error::InvalidSignatureOptions(_) => napi::Error::new(napi::Status::InvalidArg, message),
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
