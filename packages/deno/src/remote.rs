#![allow(dead_code)]

use crate::cancellation::{WvbCancellation, cancellation_of};
use crate::error::ErrorCode;
use crate::result::{WvbResult, core_err, err_result, null_handle_err, ok_handle, ok_result};
use crate::signature::SignatureVerifyKey;
use crate::{cstr, runtime};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::ffi::c_char;
use std::path::Path;
use std::sync::Arc;
use wvb::http;
use wvb::remote;

/// HTTP client options for a remote.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HttpOptions {
  /// Headers sent with every request.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub default_headers: Option<HashMap<String, String>>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  /// User-Agent sent with every request.
  pub user_agent: Option<String>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  /// Total request timeout in milliseconds.
  pub timeout: Option<u32>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  /// Response-body read timeout in milliseconds.
  pub read_timeout: Option<u32>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  /// Connection-establishment timeout in milliseconds.
  pub connect_timeout: Option<u32>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  /// Idle lifetime in milliseconds for pooled connections.
  pub pool_idle_timeout: Option<u32>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  /// Maximum idle connections retained for one host.
  pub pool_max_idle_per_host: Option<u32>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  /// Whether redirect requests include a Referer header.
  pub referer: Option<bool>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  /// Whether TCP sockets disable Nagle's algorithm.
  pub tcp_nodelay: Option<bool>,
}

impl TryFrom<HttpOptions> for remote::HttpOptions {
  type Error = String;

  fn try_from(value: HttpOptions) -> Result<Self, Self::Error> {
    let mut options = Self::new();
    if let Some(default_headers) = value.default_headers {
      let mut headers = http::HeaderMap::with_capacity(default_headers.len());
      for (name, value) in default_headers {
        let name = http::HeaderName::from_bytes(name.as_bytes())
          .map_err(|_| format!("invalid header name: {name:?}"))?;
        let value = http::HeaderValue::from_str(&value)
          .map_err(|_| format!("invalid header value: {value:?}"))?;
        headers.insert(name, value);
      }
      options = options.default_headers(headers);
    }
    if let Some(user_agent) = value.user_agent {
      options = options.user_agent(user_agent);
    }
    if let Some(timeout) = value.timeout {
      options = options.timeout(timeout as u64);
    }
    if let Some(read_timeout) = value.read_timeout {
      options = options.read_timeout(read_timeout as u64);
    }
    if let Some(connect_timeout) = value.connect_timeout {
      options = options.connect_timeout(connect_timeout as u64);
    }
    if let Some(pool_idle_timeout) = value.pool_idle_timeout {
      options = options.pool_idle_timeout(pool_idle_timeout as u64);
    }
    if let Some(pool_max_idle_per_host) = value.pool_max_idle_per_host {
      options = options.pool_max_idle_per_host(pool_max_idle_per_host as usize);
    }
    if let Some(referer) = value.referer {
      options = options.referer(referer);
    }
    if let Some(tcp_nodelay) = value.tcp_nodelay {
      options = options.tcp_nodelay(tcp_nodelay);
    }
    Ok(options)
  }
}

/// `@wvb/node`'s `RemoteConfig` minus `onDownload`: Deno FFI cannot call back into JS from the
/// worker thread a `nonblocking` symbol runs on, so download progress is not reported here.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteConfig {
  /// Base URL of the update service.
  pub base_url: String,
  /// Optional HTTP client settings.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub http: Option<HttpOptions>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteGetUpdateOptions {
  /// The etag of the update previously received; sent as `if-none-match`.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub etag: Option<String>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  /// Release channel to request.
  pub channel: Option<String>,
  /// Require the response to be signed by this key.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub expect_signature: Option<SignatureVerifyKey>,
}

impl TryFrom<RemoteGetUpdateOptions> for remote::RemoteGetUpdateOptions {
  type Error = String;

  fn try_from(value: RemoteGetUpdateOptions) -> Result<Self, Self::Error> {
    let mut options = Self::default();
    if let Some(etag) = value.etag {
      options = options.etag(etag);
    }
    if let Some(channel) = value.channel {
      options = options.channel(channel);
    }
    if let Some(expect_signature) = value.expect_signature {
      options = options.expect_signature((&expect_signature).try_into()?);
    }
    Ok(options)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
/// One bundle advertised by an update document.
pub struct BundleUpdate {
  /// Bundle name.
  pub name: String,
  /// Bundle version.
  pub version: String,
  /// Absolute download URL when the service overrides the default endpoint.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub download_url: Option<String>,
  /// Serialized integrity value for the downloaded bundle.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub integrity: Option<String>,
  /// Provider-defined, string-valued bundle metadata.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub metadata: Option<HashMap<String, String>>,
}

impl From<remote::BundleUpdate> for BundleUpdate {
  fn from(value: remote::BundleUpdate) -> Self {
    Self {
      name: value.name,
      version: value.version,
      download_url: value.download_url,
      integrity: value.integrity,
      metadata: value.metadata,
    }
  }
}

impl From<BundleUpdate> for remote::BundleUpdate {
  fn from(value: BundleUpdate) -> Self {
    Self {
      name: value.name,
      version: value.version,
      download_url: value.download_url,
      integrity: value.integrity,
      metadata: value.metadata,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
/// An atomically published set of bundle updates.
pub struct Update {
  /// Unique update identifier.
  pub id: String,
  /// ISO 8601 publication time.
  pub created_at: String,
  /// Update-model version required to process this document.
  pub runtime_version: u8,
  /// Bundles included in this update.
  pub bundles: Vec<BundleUpdate>,
  /// Provider-defined, string-valued update metadata.
  pub metadata: HashMap<String, String>,
}

impl From<remote::Update> for Update {
  fn from(value: remote::Update) -> Self {
    Self {
      id: value.id,
      created_at: value.created_at,
      runtime_version: value.runtime_version,
      bundles: value.bundles.into_iter().map(Into::into).collect(),
      metadata: value.metadata,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
/// Signature metadata returned with an update document.
pub struct UpdateSignature {
  /// Identifier of the public key used to verify the signature.
  pub key_id: String,
  /// Base64-encoded signature of the raw update document.
  pub sig: String,
  /// Signature algorithm used for this signature.
  pub alg: String,
}

impl From<remote::UpdateSignature> for UpdateSignature {
  fn from(value: remote::UpdateSignature) -> Self {
    Self {
      key_id: value.key_id,
      sig: value.sig,
      alg: value.alg,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
/// A successful update response returned by the remote service.
pub struct RemoteUpdateResponse {
  /// Parsed update document.
  pub update: Update,
  /// HTTP entity tag supplied by the remote service.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub etag: Option<String>,
  /// Signature metadata supplied by the remote service.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub signature: Option<UpdateSignature>,
}

impl From<remote::RemoteUpdateResponse> for RemoteUpdateResponse {
  fn from(value: remote::RemoteUpdateResponse) -> Self {
    Self {
      update: value.update.into(),
      etag: value.etag,
      signature: value.signature.map(Into::into),
    }
  }
}

pub struct WvbRemote {
  pub(crate) inner: Arc<remote::Remote>,
}

/// Create a remote client from `{ baseUrl, http? }`.
///
/// # Safety
/// `config_json` must be a valid NUL-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_remote_new(config_json: *const c_char) -> *mut WvbResult {
  let raw = unsafe { cstr(config_json) };
  let config: RemoteConfig = match serde_json::from_str(&raw) {
    Ok(config) => config,
    Err(e) => {
      return err_result(
        ErrorCode::InvalidRequest,
        format!("invalid remote config: {e}"),
      );
    }
  };
  let mut builder = remote::Remote::builder().base_url(config.base_url);
  if let Some(http) = config.http {
    match remote::HttpOptions::try_from(http) {
      Ok(http) => builder = builder.http(http),
      Err(message) => return err_result(ErrorCode::InvalidRequest, message),
    }
  }
  match builder.build() {
    Ok(remote) => ok_handle(Box::into_raw(Box::new(WvbRemote {
      inner: Arc::new(remote),
    }))),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be null or a pointer previously returned by `wvb_remote_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_remote_free(handle: *mut WvbRemote) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}

/// Fetch the update document. `options_json` is null/empty or a `RemoteGetUpdateOptions` object.
/// A `304 Not Modified` answers with `null`.
///
/// # Safety
/// `handle` must be a valid `WvbRemote`; `options_json` null or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_remote_get_update(
  handle: *const WvbRemote,
  options_json: *const c_char,
) -> *mut WvbResult {
  let Some(remote) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("remote");
  };
  let raw = unsafe { cstr(options_json) };
  let options = match parse_get_update_options(&raw) {
    Ok(options) => options,
    Err(result) => return result,
  };
  match runtime().block_on(async move { remote.get_update(options).await }) {
    Ok(response) => match serde_json::to_value(response.map(RemoteUpdateResponse::from)) {
      Ok(json) => ok_result(json, Vec::new()),
      Err(e) => err_result(ErrorCode::CoreSerdeJson, e.to_string()),
    },
    Err(e) => core_err(e),
  }
}

fn parse_get_update_options(
  raw: &str,
) -> Result<Option<remote::RemoteGetUpdateOptions>, *mut WvbResult> {
  if raw.is_empty() {
    return Ok(None);
  }
  let options: RemoteGetUpdateOptions = serde_json::from_str(raw).map_err(|e| {
    err_result(
      ErrorCode::InvalidRequest,
      format!("invalid get update options: {e}"),
    )
  })?;
  let options = remote::RemoteGetUpdateOptions::try_from(options)
    .map_err(|message| err_result(ErrorCode::InvalidSignatureKey, message))?;
  Ok(Some(options))
}

/// Download `url` into `filepath`. Cancelling `cancellation` fails the call with `core.cancelled`.
///
/// # Safety
/// `handle` must be a valid `WvbRemote`; `url`/`filepath` valid C strings; `cancellation` null or a
/// valid `WvbCancellation`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_remote_download(
  handle: *const WvbRemote,
  url: *const c_char,
  filepath: *const c_char,
  cancellation: *const WvbCancellation,
) -> *mut WvbResult {
  let Some(remote) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("remote");
  };
  let url = unsafe { cstr(url) };
  let filepath = unsafe { cstr(filepath) };
  let cancellation = unsafe { cancellation_of(cancellation) };
  match runtime().block_on(async move {
    remote
      .download(url, Path::new(&filepath), cancellation)
      .await
  }) {
    Ok(()) => ok_result(serde_json::Value::Null, Vec::new()),
    Err(e) => core_err(e),
  }
}
