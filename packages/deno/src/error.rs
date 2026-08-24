/// Expands the [`src/error_codes.rs`] table into [`ErrorCode`]: the enum every error result is
/// tagged with, and the wire code each variant carries. `build.rs` expands the same table into the
/// `ErrorCode` string union in `lib/error-codes.ts`.
macro_rules! error_codes {
  ($($(#[$attr:meta])* $variant:ident => $value:literal),+ $(,)?) => {
    /// The stable code every error thrown by this binding carries (`WebviewBundleError.code`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ErrorCode {
      $(
        $(#[$attr])*
        $variant,
      )+
    }

    impl ErrorCode {
      pub fn as_str(&self) -> &'static str {
        match self {
          $(Self::$variant => $value,)+
        }
      }
    }
  };
}

include!("error_codes.rs");

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
