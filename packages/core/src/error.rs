#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
  Io,
  Compress,
  Decompress,
  Encode,
  Decode,
  Http,
  InvalidMagicNum,
  InvalidVersion,
  InvalidHeaderChecksum,
  InvalidIndexChecksum,
  ChecksumMismatch,
  BundleNotFound,
  BundleEntryNotExists,
  BundleCannotBeRemoved,
  InvalidFilepath,
  SerdeJson,
  CannotResolveLocalHost,
  Reqwest,
  InvalidRemoteUrl,
  InvalidRemoteBundle,
  RemoteBundleNotFound,
  RemoteForbidden,
  RemoteHttp,
  InvalidRemoteConfig,
  InvalidIntegrity,
  IntegrityRequired,
  IntegrityVerifyFailed,
  InvalidSignature,
  InvalidSigningKey,
  SignatureSignFailed,
  InvalidVerifyingKey,
  SignatureNotExists,
  SignatureVerifyFailed,
  Generic,
}

impl ErrorCode {
  /// The wire form of the code, as exposed to every binding.
  pub const fn as_str(&self) -> &'static str {
    match self {
      Self::Io => "io",
      Self::Compress => "compress",
      Self::Decompress => "decompress",
      Self::Encode => "encode",
      Self::Decode => "decode",
      Self::Http => "http",
      Self::InvalidMagicNum => "invalid_magic_num",
      Self::InvalidVersion => "invalid_version",
      Self::InvalidHeaderChecksum => "invalid_header_checksum",
      Self::InvalidIndexChecksum => "invalid_index_checksum",
      Self::ChecksumMismatch => "checksum_mismatch",
      Self::BundleNotFound => "bundle_not_found",
      Self::BundleEntryNotExists => "bundle_entry_not_exists",
      Self::BundleCannotBeRemoved => "bundle_cannot_be_removed",
      Self::InvalidFilepath => "invalid_filepath",
      Self::SerdeJson => "serde_json",
      Self::CannotResolveLocalHost => "cannot_resolve_local_host",
      Self::Reqwest => "reqwest",
      Self::InvalidRemoteUrl => "invalid_remote_url",
      Self::InvalidRemoteBundle => "invalid_remote_bundle",
      Self::RemoteBundleNotFound => "remote_bundle_not_found",
      Self::RemoteForbidden => "remote_forbidden",
      Self::RemoteHttp => "remote_http",
      Self::InvalidRemoteConfig => "invalid_remote_config",
      Self::InvalidIntegrity => "invalid_integrity",
      Self::IntegrityRequired => "integrity_required",
      Self::IntegrityVerifyFailed => "integrity_verify_failed",
      Self::InvalidSignature => "invalid_signature",
      Self::InvalidSigningKey => "invalid_signing_key",
      Self::SignatureSignFailed => "signature_sign_failed",
      Self::InvalidVerifyingKey => "invalid_verifying_key",
      Self::SignatureNotExists => "signature_not_exists",
      Self::SignatureVerifyFailed => "signature_verify_failed",
      Self::Generic => "generic",
    }
  }
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
  pub fn code(&self) -> ErrorCode {
    match self {
      Self::Io(_) => ErrorCode::Io,
      Self::Compress(_) => ErrorCode::Compress,
      Self::Decompress(_) => ErrorCode::Decompress,
      Self::Encode { .. } => ErrorCode::Encode,
      Self::Decode { .. } => ErrorCode::Decode,
      Self::Http(_) => ErrorCode::Http,
      Self::InvalidMagicNum => ErrorCode::InvalidMagicNum,
      Self::InvalidVersion => ErrorCode::InvalidVersion,
      Self::InvalidHeaderChecksum => ErrorCode::InvalidHeaderChecksum,
      Self::InvalidIndexChecksum => ErrorCode::InvalidIndexChecksum,
      Self::ChecksumMismatch => ErrorCode::ChecksumMismatch,
      Self::BundleNotFound => ErrorCode::BundleNotFound,
      #[cfg(feature = "source")]
      Self::BundleEntryNotExists { .. } => ErrorCode::BundleEntryNotExists,
      #[cfg(feature = "source")]
      Self::BundleCannotBeRemoved { .. } => ErrorCode::BundleCannotBeRemoved,
      #[cfg(feature = "source")]
      Self::InvalidFilepath(_) => ErrorCode::InvalidFilepath,
      #[cfg(feature = "_serde")]
      Self::SerdeJson(_) => ErrorCode::SerdeJson,
      #[cfg(feature = "protocol-local")]
      Self::CannotResolveLocalHost => ErrorCode::CannotResolveLocalHost,
      #[cfg(feature = "_reqwest")]
      Self::Reqwest(_) => ErrorCode::Reqwest,
      #[cfg(feature = "remote")]
      Self::InvalidRemoteUrl(_) => ErrorCode::InvalidRemoteUrl,
      #[cfg(feature = "remote")]
      Self::InvalidRemoteBundle(_) => ErrorCode::InvalidRemoteBundle,
      #[cfg(feature = "remote")]
      Self::RemoteBundleNotFound => ErrorCode::RemoteBundleNotFound,
      #[cfg(feature = "remote")]
      Self::RemoteForbidden => ErrorCode::RemoteForbidden,
      #[cfg(feature = "remote")]
      Self::RemoteHttp { .. } => ErrorCode::RemoteHttp,
      #[cfg(feature = "remote")]
      Self::InvalidRemoteConfig(_) => ErrorCode::InvalidRemoteConfig,
      #[cfg(feature = "integrity")]
      Self::InvalidIntegrity(_) => ErrorCode::InvalidIntegrity,
      #[cfg(feature = "integrity")]
      Self::IntegrityRequired => ErrorCode::IntegrityRequired,
      #[cfg(feature = "integrity")]
      Self::IntegrityVerifyFailed => ErrorCode::IntegrityVerifyFailed,
      #[cfg(feature = "signature")]
      Self::InvalidSignature => ErrorCode::InvalidSignature,
      #[cfg(feature = "signature")]
      Self::InvalidSigningKey(_) => ErrorCode::InvalidSigningKey,
      #[cfg(feature = "signature")]
      Self::SignatureSignFailed(_) => ErrorCode::SignatureSignFailed,
      #[cfg(feature = "signature")]
      Self::InvalidVerifyingKey(_) => ErrorCode::InvalidVerifyingKey,
      #[cfg(feature = "signature")]
      Self::SignatureNotExists => ErrorCode::SignatureNotExists,
      #[cfg(feature = "signature")]
      Self::SignatureVerifyFailed => ErrorCode::SignatureVerifyFailed,
      Self::Generic(_) => ErrorCode::Generic,
    }
  }

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
