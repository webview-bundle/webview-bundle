/// Declares [`ErrorCode`] and its mapping from [`Error`] out of one table, so a new error
/// variant cannot be added without also giving it a wire code.
macro_rules! error_codes {
  (
    $(
      $(#[cfg($cfg:meta)])?
      $variant:ident => $wire:literal,
    )*
  ) => {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum ErrorCode {
      $($variant,)*
    }

    impl ErrorCode {
      /// The wire form of the code, as exposed to every binding.
      pub const fn as_str(&self) -> &'static str {
        match self {
          $(Self::$variant => $wire,)*
        }
      }
    }

    impl Error {
      pub fn code(&self) -> ErrorCode {
        match self {
          $(
            $(#[cfg($cfg)])?
            Self::$variant { .. } => ErrorCode::$variant,
          )*
        }
      }
    }
  };
}

error_codes! {
  Io => "io",
  Compress => "compress",
  Decompress => "decompress",
  Encode => "encode",
  Decode => "decode",
  Http => "http",
  HttpInvalidUri => "http_invalid_uri",
  Cancelled => "cancelled",
  Timeout => "timeout",
  InvalidMagicNum => "invalid_magic_num",
  InvalidVersion => "invalid_version",
  InvalidHeaderChecksum => "invalid_header_checksum",
  InvalidIndexChecksum => "invalid_index_checksum",
  ChecksumMismatch => "checksum_mismatch",
  BundleNotFound => "bundle_not_found",
  #[cfg(feature = "source")]
  BundleEntryNotExists => "bundle_entry_not_exists",
  #[cfg(feature = "source")]
  BundleCannotBeRemoved => "bundle_cannot_be_removed",
  #[cfg(feature = "source")]
  InvalidFilepath => "invalid_filepath",
  #[cfg(feature = "_serde")]
  SerdeJson => "serde_json",
  #[cfg(feature = "protocol-proxy")]
  CannotResolveProxyServer => "cannot_resolve_proxy_server",
  #[cfg(feature = "_reqwest")]
  HttpClient => "http_client",
  #[cfg(feature = "remote")]
  RemoteHttp => "remote_http",
  #[cfg(feature = "remote")]
  BadRemoteRequest => "bad_remote_request",
  #[cfg(feature = "remote")]
  BadRemoteResponse => "bad_remote_response",
  #[cfg(feature = "remote")]
  InvalidRemoteConfig => "invalid_remote_config",
  #[cfg(feature = "updater")]
  InvalidUpdaterConfig => "invalid_updater_config",
  #[cfg(feature = "updater")]
  InstallAtomicFailed => "install_atomic_failed",
  #[cfg(feature = "integrity")]
  InvalidIntegrity => "invalid_integrity",
  #[cfg(feature = "integrity")]
  IntegrityRequired => "integrity_required",
  #[cfg(feature = "integrity")]
  IntegrityVerifyFailed => "integrity_verify_failed",
  #[cfg(feature = "signature")]
  InvalidSignature => "invalid_signature",
  #[cfg(feature = "signature")]
  InvalidSignatureKey => "invalid_signature_key",
  #[cfg(feature = "signature")]
  ExpectSignatureNotFound => "expect_signature_not_found",
  #[cfg(feature = "signature")]
  SignatureVerifyFailed => "signature_verify_failed",
  Generic => "generic",
}

impl std::fmt::Display for ErrorCode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.as_str())
  }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("io error: {0}")]
  Io(#[from] std::io::Error),
  #[error("compress error: {0}")]
  Compress(#[from] lz4_flex::block::CompressError),
  #[error("decompress error: {0}")]
  Decompress(#[from] lz4_flex::block::DecompressError),
  #[error("encode error: {message}")]
  Encode {
    #[source]
    error: bincode::error::EncodeError,
    message: String,
  },
  #[error("decode error: {message}")]
  Decode {
    #[source]
    error: bincode::error::DecodeError,
    message: String,
  },
  #[error("http error: {0}")]
  Http(#[from] http::Error),
  #[error("http invalid uri: {0}")]
  HttpInvalidUri(#[from] http::uri::InvalidUri),
  #[error("cancelled")]
  Cancelled,
  #[error("timeout")]
  Timeout,
  #[error("invalid magic number")]
  InvalidMagicNum,
  #[error("invalid version format")]
  InvalidVersion,
  #[error("invalid header checksum")]
  InvalidHeaderChecksum,
  #[error("invalid index checksum")]
  InvalidIndexChecksum,
  #[error("checksum mismatch")]
  ChecksumMismatch,
  #[error("bundle not found")]
  BundleNotFound,
  #[cfg(feature = "source")]
  #[error("bundle entry not exists (bundle_name: {bundle_name}, version: {version})")]
  BundleEntryNotExists {
    bundle_name: String,
    version: String,
  },
  #[cfg(feature = "source")]
  #[error("bundle cannot be removed (bundle_name: {bundle_name}, version: {version})")]
  BundleCannotBeRemoved {
    bundle_name: String,
    version: String,
  },
  #[cfg(feature = "source")]
  #[error("invalid filepath: {0:?}")]
  InvalidFilepath(String),
  #[cfg(feature = "_serde")]
  #[error("serde json error: {0}")]
  SerdeJson(#[from] serde_json::Error),
  #[cfg(feature = "protocol-proxy")]
  #[error("cannot resolve proxy server")]
  CannotResolveProxyServer,
  #[cfg(feature = "_reqwest")]
  #[error("http client error: {0}")]
  HttpClient(#[from] reqwest::Error),
  #[cfg(feature = "remote")]
  #[error("remote http error with status {status}: {}", .message.as_deref().unwrap_or("unknown"))]
  RemoteHttp {
    status: u16,
    message: Option<String>,
  },
  #[cfg(feature = "remote")]
  #[error("bad remote request: {0}")]
  BadRemoteRequest(String),
  #[cfg(feature = "remote")]
  #[error("bad remote response: {0}")]
  BadRemoteResponse(String),
  #[cfg(feature = "remote")]
  #[error("invalid remote config: {0}")]
  InvalidRemoteConfig(String),
  #[cfg(feature = "updater")]
  #[error("invalid updater config: {0}")]
  InvalidUpdaterConfig(String),
  #[cfg(feature = "updater")]
  #[error("install failed atomically (bundle_name: {bundle_name}, version: {version})")]
  InstallAtomicFailed {
    bundle_name: String,
    version: String,
  },
  #[cfg(feature = "integrity")]
  #[error("invalid integrity: {0}")]
  InvalidIntegrity(String),
  #[cfg(feature = "integrity")]
  #[error("integrity required")]
  IntegrityRequired,
  #[cfg(feature = "integrity")]
  #[error("integrity verify failed")]
  IntegrityVerifyFailed,
  #[cfg(feature = "signature")]
  #[error("invalid signature")]
  InvalidSignature,
  #[cfg(feature = "signature")]
  #[error("invalid verifying key: {0}")]
  InvalidSignatureKey(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
  #[cfg(feature = "signature")]
  #[error("expect signature not found: key_id={key_id}")]
  ExpectSignatureNotFound { key_id: String },
  #[cfg(feature = "signature")]
  #[error("signature verify failed")]
  SignatureVerifyFailed,
  #[error("generic error: {0}")]
  Generic(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl Error {
  #[cfg(feature = "source")]
  pub(crate) fn bundle_entry_not_exists(
    bundle_name: impl Into<String>,
    version: impl Into<String>,
  ) -> Self {
    Self::BundleEntryNotExists {
      bundle_name: bundle_name.into(),
      version: version.into(),
    }
  }

  #[cfg(feature = "source")]
  pub(crate) fn bundle_cannot_be_removed(
    bundle_name: impl Into<String>,
    version: impl Into<String>,
  ) -> Self {
    Self::BundleCannotBeRemoved {
      bundle_name: bundle_name.into(),
      version: version.into(),
    }
  }

  #[cfg(feature = "source")]
  pub(crate) fn invalid_filepath(filepath: impl Into<String>) -> Self {
    Self::InvalidFilepath(filepath.into())
  }

  #[cfg(feature = "remote")]
  pub(crate) fn invalid_remote_config(message: impl Into<String>) -> Self {
    Self::InvalidRemoteConfig(message.into())
  }

  #[cfg(feature = "remote")]
  pub(crate) fn remote_http(status: http::StatusCode, message: Option<impl Into<String>>) -> Self {
    Self::RemoteHttp {
      status: status.as_u16(),
      message: message.map(|x| x.into()),
    }
  }

  #[cfg(feature = "remote")]
  pub(crate) fn bad_remote_request(message: impl Into<String>) -> Self {
    Self::BadRemoteRequest(message.into())
  }

  #[cfg(feature = "remote")]
  pub(crate) fn bad_remote_response(message: impl Into<String>) -> Self {
    Self::BadRemoteResponse(message.into())
  }

  #[cfg(feature = "updater")]
  pub(crate) fn invalid_updater_config(message: impl Into<String>) -> Self {
    Self::InvalidUpdaterConfig(message.into())
  }

  #[cfg(feature = "updater")]
  pub(crate) fn install_atomic_failed(
    bundle_name: impl Into<String>,
    version: impl Into<String>,
  ) -> Self {
    Self::InstallAtomicFailed {
      bundle_name: bundle_name.into(),
      version: version.into(),
    }
  }

  #[cfg(feature = "integrity")]
  pub(crate) fn invalid_integrity(message: impl Into<String>) -> Self {
    Self::InvalidIntegrity(message.into())
  }

  #[cfg(feature = "signature")]
  pub(crate) fn invalid_signature_key(
    error: impl Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
  ) -> Self {
    Self::InvalidSignatureKey(error.into())
  }

  #[cfg(feature = "signature")]
  pub(crate) fn expect_signature_not_found(key_id: impl Into<String>) -> Self {
    Self::ExpectSignatureNotFound {
      key_id: key_id.into(),
    }
  }

  #[allow(dead_code)]
  pub(crate) fn generic(
    error: impl Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
  ) -> Self {
    Self::Generic(error.into())
  }
}
