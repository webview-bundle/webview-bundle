use napi::Env;
use napi::bindgen_prelude::{
  JsObjectValue, Object, Property, PropertyAttributes, ToNapiValue, Unknown,
};
use napi_derive::napi;
use std::future::Future;
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

    impl AsRef<str> for ErrorCode {
      fn as_ref(&self) -> &'static str {
        self.as_str()
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

const ERROR_NAME: &str = "WebviewBundleError";

/// Creates a coded JavaScript error while napi is on the JS thread. This preserves custom codes
/// for rejected promises, which napi otherwise reduces to a generic status error.
pub(crate) fn js_error(env: napi::sys::napi_env, error: Error) -> napi::Error {
  let raw = unsafe { napi::JsError::<ErrorCode>::from(error).into_value(env) };
  brand_name(env, raw);
  napi::Error::from(unsafe { Unknown::from_raw_unchecked(env, raw) })
}

impl From<Error> for napi::JsError<ErrorCode> {
  fn from(value: Error) -> Self {
    let code = value.code();
    napi::JsError::from(napi::Error::new(code, value.to_string()))
  }
}

fn brand_name(raw_env: napi::sys::napi_env, raw: napi::sys::napi_value) {
  let env = Env::from_raw(raw_env);
  let mut object = Object::from_raw(raw_env, raw);
  let Ok(name) = env.create_string(ERROR_NAME).and_then(|value| {
    Property::new()
      .with_utf8_name("name")
      .map(|property| property.with_value(&value))
  }) else {
    return;
  };
  let _ = object
    .define_properties(&[name
      .with_property_attributes(PropertyAttributes::Configurable | PropertyAttributes::Writable)]);
}

/// Defers a Rust failure until napi can construct the actual JavaScript error object.
pub struct Outcome<T>(pub crate::Result<T>);

impl<T> Outcome<T> {
  pub fn from_fn(f: impl FnOnce() -> crate::Result<T>) -> Self {
    Self(f())
  }

  pub async fn from_future(future: impl Future<Output = crate::Result<T>>) -> Self {
    Self(future.await)
  }

  pub fn into_napi(self, env: Env) -> napi::Result<T> {
    self.0.map_err(|error| js_error(env.raw(), error))
  }
}

impl<T: ToNapiValue> ToNapiValue for Outcome<T> {
  unsafe fn to_napi_value(
    env: napi::sys::napi_env,
    value: Self,
  ) -> napi::Result<napi::sys::napi_value> {
    match value.0 {
      Ok(value) => unsafe { T::to_napi_value(env, value) },
      Err(error) => Err(js_error(env, error)),
    }
  }
}
