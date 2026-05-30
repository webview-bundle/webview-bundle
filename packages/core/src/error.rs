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
  #[error("bundle cannot be removed (bundle_name: {bundle_name}, version: {version}): {reason}")]
  BundleCannotBeRemoved {
    bundle_name: String,
    version: String,
    reason: String,
  },
  #[cfg(feature = "source")]
  #[error("invalid filepath: {0:?}")]
  InvalidFilepath(String),
  #[cfg(feature = "_serde")]
  #[error("serde json error: {0}")]
  SerdeJson(#[from] serde_json::Error),
  #[cfg(feature = "protocol-local")]
  #[error("cannot resolve local host")]
  CannotResolveLocalHost,
  #[cfg(feature = "_reqwest")]
  #[error("reqwest error: {0}")]
  Reqwest(#[from] reqwest::Error),
  #[cfg(feature = "remote")]
  #[error("invalid remote url: {0}")]
  InvalidRemoteUrl(#[from] http::uri::InvalidUri),
  #[cfg(feature = "remote")]
  #[error("invalid remote bundle: {0}")]
  InvalidRemoteBundle(String),
  #[cfg(feature = "remote")]
  #[error("remote bundle not found")]
  RemoteBundleNotFound,
  #[cfg(feature = "remote")]
  #[error("remote forbidden")]
  RemoteForbidden,
  #[cfg(feature = "remote")]
  #[error("remote http error with status {status}")]
  RemoteHttp {
    status: u16,
    message: Option<String>,
  },
  #[cfg(feature = "remote")]
  #[error("invalid remote config: {0}")]
  InvalidRemoteConfig(String),
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
  #[error("invalid signing key: {0}")]
  InvalidSigningKey(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
  #[cfg(feature = "signature")]
  #[error("signature sign failed: {0}")]
  SignatureSignFailed(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
  #[cfg(feature = "signature")]
  #[error("invalid verifying key: {0}")]
  InvalidVerifyingKey(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
  #[cfg(feature = "signature")]
  #[error("signature not exists")]
  SignatureNotExists,
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
    reason: impl Into<String>,
  ) -> Self {
    Self::BundleCannotBeRemoved {
      bundle_name: bundle_name.into(),
      version: version.into(),
      reason: reason.into(),
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
  pub(crate) fn invalid_remote_bundle(message: impl Into<String>) -> Self {
    Self::InvalidRemoteBundle(message.into())
  }

  #[cfg(feature = "remote")]
  pub(crate) fn remote_http(status: http::StatusCode, message: Option<impl Into<String>>) -> Self {
    Self::RemoteHttp {
      status: status.as_u16(),
      message: message.map(|x| x.into()),
    }
  }

  #[cfg(feature = "integrity")]
  pub(crate) fn invalid_integrity(message: impl Into<String>) -> Self {
    Self::InvalidIntegrity(message.into())
  }

  #[cfg(feature = "signature")]
  pub(crate) fn invalid_verifying_key(
    error: impl Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
  ) -> Self {
    Self::InvalidVerifyingKey(error.into())
  }

  #[allow(dead_code)]
  pub(crate) fn generic(
    error: impl Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
  ) -> Self {
    Self::Generic(error.into())
  }
}
