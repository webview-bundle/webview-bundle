use napi::Env;
use napi::bindgen_prelude::{
  JsObjectValue, Object, Property, PropertyAttributes, ToNapiValue, Unknown,
};
use napi_derive::napi;
use std::future::Future;
use wvb::http;

macro_rules! error_codes {
  ($($(#[$attr:meta])* $variant:ident => $value:literal),+ $(,)?) => {
    /// The stable `code` every error thrown by this binding carries.
    #[napi(string_enum)]
    pub enum WebviewBundleErrorCode {
      $(
        $(#[$attr])*
        #[napi(value = $value)]
        $variant,
      )+
    }

    // `as_ref` becomes the thrown error's JS `code`.
    impl AsRef<str> for WebviewBundleErrorCode {
      fn as_ref(&self) -> &'static str {
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
impl From<wvb::ErrorCode> for WebviewBundleErrorCode {
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
  // Only the reason reaches JS; the default Display would prefix the napi status.
  #[error("{}", .0.reason)]
  Napi(#[from] napi::Error),
}

impl Error {
  pub(crate) fn invalid_signature_options(message: impl Into<String>) -> Self {
    Self::InvalidSignatureOptions(message.into())
  }

  pub(crate) fn code(&self) -> WebviewBundleErrorCode {
    match self {
      Self::Core(e) => e.code().into(),
      Self::InvalidHeaderName(_) => WebviewBundleErrorCode::InvalidHeaderName,
      Self::InvalidHeaderValue(_) => WebviewBundleErrorCode::InvalidHeaderValue,
      Self::InvalidSignatureOptions(_) => WebviewBundleErrorCode::InvalidSignatureOptions,
      Self::Napi(_) => WebviewBundleErrorCode::Napi,
    }
  }
}

impl From<Error> for napi::JsError<WebviewBundleErrorCode> {
  fn from(value: Error) -> Self {
    napi::JsError::from(napi::Error::new(value.code(), value.to_string()))
  }
}

const ERROR_NAME: &str = "WebviewBundleError";

/// Builds the coded, branded JS `Error` and wraps it so napi throws it verbatim.
/// Must run on the JS thread: the result owns a reference to a JS object.
pub(crate) fn js_error(env: napi::sys::napi_env, error: Error) -> napi::Error {
  let raw = unsafe { napi::JsError::from(error).into_value(env) };
  brand_name(env, raw);
  napi::Error::from(unsafe { Unknown::from_raw_unchecked(env, raw) })
}

/// Defines (not assigns) an own `name`, so a frozen `Error.prototype` can't swallow the brand.
fn brand_name(raw_env: napi::sys::napi_env, raw: napi::sys::napi_value) {
  let env = Env::from_raw(raw_env);
  let mut object = Object::from_raw(raw_env, raw);
  let Ok(name) = env.create_string(ERROR_NAME).and_then(|value| {
    Property::new()
      .with_utf8_name("name")
      .map(|p| p.with_value(&value))
  }) else {
    return;
  };
  let _ = object
    .define_properties(&[name
      .with_property_attributes(PropertyAttributes::Configurable | PropertyAttributes::Writable)]);
}

/// Carries a failure across the napi boundary as a value so its `ToNapiValue` (run by napi on the
/// JS thread) can build a coded `Error`. napi only writes a custom `code` for *synchronous* throws;
/// an async rejection is always a napi status, and naming the return type anything but `Result`
/// keeps napi off the error channel. <https://github.com/napi-rs/napi-rs/issues/2178>
pub struct Outcome<T>(pub crate::Result<T>);

impl<T> Outcome<T> {
  pub fn from_fn(f: impl FnOnce() -> crate::Result<T>) -> Self {
    Self(f())
  }

  pub async fn from_future(future: impl Future<Output = crate::Result<T>>) -> Self {
    Self(future.await)
  }

  /// For a `#[napi(constructor)]`, which must return the class instance rather than an `Outcome`.
  pub fn into_napi(self, env: Env) -> napi::Result<T> {
    self.0.map_err(|error| js_error(env.raw(), error))
  }
}

impl<T: ToNapiValue> ToNapiValue for Outcome<T> {
  unsafe fn to_napi_value(
    env: napi::sys::napi_env,
    val: Self,
  ) -> napi::Result<napi::sys::napi_value> {
    match val.0 {
      Ok(value) => unsafe { T::to_napi_value(env, value) },
      Err(error) => Err(js_error(env, error)),
    }
  }
}
