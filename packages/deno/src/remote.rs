use crate::result::{WvbResult, core_err, null_handle_err, ok_result, wire_json};
use crate::{cstr, runtime};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::ffi::c_char;
use std::sync::Arc;
use wvb::http;
use wvb::remote::{self, HttpOptions, Remote as CoreRemote, RemoteFetchOptions};

/// Bundle list info from the remote server.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListRemoteBundleInfo {
  pub name: String,
  pub version: String,
}

impl From<&remote::ListRemoteBundleInfo> for ListRemoteBundleInfo {
  fn from(info: &remote::ListRemoteBundleInfo) -> Self {
    Self {
      name: info.name.clone(),
      version: info.version.clone(),
    }
  }
}

/// Bundle info from the remote server.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBundleInfo {
  pub name: String,
  pub version: String,
  #[specta(optional)]
  pub etag: Option<String>,
  #[specta(optional)]
  pub integrity: Option<String>,
  #[specta(optional)]
  pub signature: Option<String>,
  #[specta(optional)]
  pub last_modified: Option<String>,
}

impl From<&remote::RemoteBundleInfo> for RemoteBundleInfo {
  fn from(info: &remote::RemoteBundleInfo) -> Self {
    Self {
      name: info.name.clone(),
      version: info.version.clone(),
      etag: info.etag.clone(),
      integrity: info.integrity.clone(),
      signature: info.signature.clone(),
      last_modified: info.last_modified.clone(),
    }
  }
}

/// Remote fetch options for a `channel` crossing the C boundary, where `""` means "no channel".
fn fetch_options(channel: &str) -> Option<RemoteFetchOptions> {
  (!channel.is_empty()).then(|| RemoteFetchOptions::default().channel(channel))
}

pub struct WvbRemote {
  pub(crate) inner: Arc<CoreRemote>,
}

pub(crate) fn list_info_json(info: &wvb::remote::ListRemoteBundleInfo) -> serde_json::Value {
  wire_json(ListRemoteBundleInfo::from(info))
}

pub(crate) fn remote_info_json(info: &wvb::remote::RemoteBundleInfo) -> serde_json::Value {
  wire_json(RemoteBundleInfo::from(info))
}

/// Parse a JSON object of HTTP client options into [`HttpOptions`].
fn parse_http_options(raw: &str) -> Option<HttpOptions> {
  let value: serde_json::Value = serde_json::from_str(raw).ok()?;
  let mut options = HttpOptions::new();
  if let Some(x) = value.get("defaultHeaders").and_then(|x| x.as_object()) {
    let mut headers = http::HeaderMap::with_capacity(x.len());
    for (name, value) in x {
      let name = http::HeaderName::from_bytes(name.as_bytes()).ok()?;
      let value = http::HeaderValue::from_str(value.as_str()?).ok()?;
      headers.insert(name, value);
    }
    options = options.default_headers(headers);
  }
  if let Some(x) = value.get("userAgent").and_then(|x| x.as_str()) {
    options = options.user_agent(x.to_string());
  }
  if let Some(x) = value.get("timeout").and_then(|x| x.as_u64()) {
    options = options.timeout(x);
  }
  if let Some(x) = value.get("readTimeout").and_then(|x| x.as_u64()) {
    options = options.read_timeout(x);
  }
  if let Some(x) = value.get("connectTimeout").and_then(|x| x.as_u64()) {
    options = options.connect_timeout(x);
  }
  if let Some(x) = value.get("poolIdleTimeout").and_then(|x| x.as_u64()) {
    options = options.pool_idle_timeout(x);
  }
  if let Some(x) = value.get("poolMaxIdlePerHost").and_then(|x| x.as_u64()) {
    options = options.pool_max_idle_per_host(x as usize);
  }
  if let Some(x) = value.get("referer").and_then(|x| x.as_bool()) {
    options = options.referer(x);
  }
  if let Some(x) = value.get("tcpNodelay").and_then(|x| x.as_bool()) {
    options = options.tcp_nodelay(x);
  }
  Some(options)
}

/// Create a remote client. `http_json` is null/empty or a JSON object of HTTP options.
///
/// # Safety
/// `endpoint` must be a valid C string; `http_json` must be null or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_remote_new(
  endpoint: *const c_char,
  http_json: *const c_char,
) -> *mut WvbRemote {
  let endpoint = unsafe { cstr(endpoint) };
  let mut builder = CoreRemote::builder().endpoint(endpoint);
  // parse_http_options returns None for an empty / unparsable string, so no separate guard is needed.
  let http_raw = unsafe { cstr(http_json) };
  if let Some(options) = parse_http_options(&http_raw) {
    builder = builder.http(options);
  }
  match builder.build() {
    Ok(remote) => Box::into_raw(Box::new(WvbRemote {
      inner: Arc::new(remote),
    })),
    Err(_) => std::ptr::null_mut(),
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

/// # Safety
/// `handle` must be a valid `WvbRemote`; `channel` null or a valid C string ("" = no channel).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_remote_list_bundles(
  handle: *const WvbRemote,
  channel: *const c_char,
) -> *mut WvbResult {
  let Some(remote) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("remote");
  };
  let channel = unsafe { cstr(channel) };
  match runtime().block_on(async move { remote.list_bundles(fetch_options(&channel)).await }) {
    Ok(list) => ok_result(
      serde_json::Value::Array(list.iter().map(list_info_json).collect()),
      Vec::new(),
    ),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbRemote`; `bundle_name`/`channel` valid C strings ("" channel = none).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_remote_get_info(
  handle: *const WvbRemote,
  bundle_name: *const c_char,
  channel: *const c_char,
) -> *mut WvbResult {
  let Some(remote) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("remote");
  };
  let name = unsafe { cstr(bundle_name) };
  let channel = unsafe { cstr(channel) };
  match runtime().block_on(async move {
    remote
      .get_current_info(&name, fetch_options(&channel))
      .await
  }) {
    Ok(info) => ok_result(remote_info_json(&info), Vec::new()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbRemote`; `bundle_name`/`channel` valid C strings ("" channel = none).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_remote_download(
  handle: *const WvbRemote,
  bundle_name: *const c_char,
  channel: *const c_char,
) -> *mut WvbResult {
  let Some(remote) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("remote");
  };
  let name = unsafe { cstr(bundle_name) };
  let channel = unsafe { cstr(channel) };
  match runtime().block_on(async move {
    let channel = (!channel.is_empty()).then_some(&channel);
    remote.download(&name, channel).await
  }) {
    Ok((info, _bundle, data)) => ok_result(remote_info_json(&info), data.to_vec()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbRemote`; `bundle_name`/`version` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_remote_download_version(
  handle: *const WvbRemote,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let Some(remote) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("remote");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  match runtime().block_on(async move { remote.download_version(&name, &version).await }) {
    Ok((info, _bundle, data)) => ok_result(remote_info_json(&info), data.to_vec()),
    Err(e) => core_err(e),
  }
}
