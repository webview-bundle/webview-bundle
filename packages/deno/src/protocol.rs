#![allow(dead_code)]

use crate::error::ErrorCode;
use crate::result::{WvbResult, core_err, err_result, null_handle_err, ok_handle, ok_result};
use crate::source::WvbSource;
use crate::{cstr, runtime};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::ffi::c_char;
use std::sync::Arc;
use wvb::http;
use wvb::protocol::{
  BundleProtocol, HostnameSegment as CoreHostnameSegment, Protocol, ProxyProtocol, ProxyResolver,
  UriBundleResolver as CoreUriBundleResolver, UriPathResolver as CoreUriPathResolver,
};

/// Which hostname segment names the bundle.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum HostnameSegment {
  First,
  Full,
  StripSuffix,
}

impl From<HostnameSegment> for CoreHostnameSegment {
  fn from(value: HostnameSegment) -> Self {
    match value {
      HostnameSegment::First => Self::First,
      HostnameSegment::Full => Self::Full,
      HostnameSegment::StripSuffix => Self::StripSuffix,
    }
  }
}

/// A named hostname segment, or the nth one.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(untagged)]
pub enum HostnameSegmentSelector {
  Named(HostnameSegment),
  Nth(u32),
}

impl From<HostnameSegmentSelector> for CoreHostnameSegment {
  fn from(value: HostnameSegmentSelector) -> Self {
    match value {
      HostnameSegmentSelector::Named(segment) => segment.into(),
      HostnameSegmentSelector::Nth(index) => Self::Nth(index as usize),
    }
  }
}

/// How the bundle name is resolved from the request uri. The TypeScript counterpart is hand-written
/// in `lib/protocol.ts`, because `segment` is a union `HostnameSegment | number`.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum UriBundleResolver {
  #[serde(rename_all = "camelCase")]
  Hostname {
    segment: Option<HostnameSegmentSelector>,
    allow_wvb_suffix_only: Option<bool>,
  },
  #[serde(rename_all = "camelCase")]
  Pathname { segment_index: Option<u32> },
}

impl From<UriBundleResolver> for CoreUriBundleResolver {
  fn from(value: UriBundleResolver) -> Self {
    match value {
      UriBundleResolver::Hostname {
        segment,
        allow_wvb_suffix_only,
      } => Self::hostname(segment.map(Into::into), allow_wvb_suffix_only),
      UriBundleResolver::Pathname { segment_index } => {
        Self::pathname(segment_index.map(|index| index as usize))
      }
    }
  }
}

/// How the file path in the bundle is resolved from the request uri.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum UriPathResolver {
  Exact,
  DirectoryIndex,
  HtmlExtension,
}

impl From<UriPathResolver> for CoreUriPathResolver {
  fn from(value: UriPathResolver) -> Self {
    match value {
      UriPathResolver::Exact => Self::exact(),
      UriPathResolver::DirectoryIndex => Self::directory_index(),
      UriPathResolver::HtmlExtension => Self::html_extension(),
    }
  }
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleProtocolOptions {
  pub bundle_resolver: Option<UriBundleResolver>,
  pub path_resolver: Option<UriPathResolver>,
}

/// HTTP method accepted by a protocol handler (case-insensitive on the wire).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum HttpMethod {
  Get,
  Head,
  Options,
  Post,
  Put,
  Patch,
  Delete,
  Trace,
  Connect,
}

pub struct WvbProtocol {
  inner: Arc<dyn Protocol>,
}

/// Create a bundle protocol over a source. `options_json` is null/empty or a
/// `BundleProtocolOptions` object; an unknown or ill-typed option fails the call rather than
/// serving requests with a setting the caller did not ask for.
///
/// # Safety
/// `source` must be a valid `WvbSource`; `options_json` null or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_bundle_protocol_new(
  source: *const WvbSource,
  options_json: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { source.as_ref() }) else {
    return null_handle_err("source");
  };
  let mut protocol = BundleProtocol::new(source.inner.clone());
  let raw = unsafe { cstr(options_json) };
  if !raw.is_empty() {
    let options: BundleProtocolOptions = match serde_json::from_str(&raw) {
      Ok(options) => options,
      Err(e) => {
        return err_result(
          ErrorCode::InvalidRequest,
          format!("invalid bundle protocol options: {e}"),
        );
      }
    };
    if let Some(bundle_resolver) = options.bundle_resolver {
      protocol = protocol.set_bundle_resolver(bundle_resolver.into());
    }
    if let Some(path_resolver) = options.path_resolver {
      protocol = protocol.set_path_resolver(path_resolver.into());
    }
  }
  let protocol: Arc<dyn Protocol> = Arc::new(protocol);
  ok_handle(Box::into_raw(Box::new(WvbProtocol { inner: protocol })))
}

/// `hosts_json` maps a custom host to the base url it is proxied to.
///
/// # Safety
/// `hosts_json` must be null or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_proxy_protocol_new(hosts_json: *const c_char) -> *mut WvbResult {
  let raw = unsafe { cstr(hosts_json) };
  let hosts: HashMap<String, String> = if raw.is_empty() {
    HashMap::new()
  } else {
    match serde_json::from_str(&raw) {
      Ok(hosts) => hosts,
      Err(e) => {
        return err_result(
          ErrorCode::InvalidRequest,
          format!("invalid proxy hosts: {e}"),
        );
      }
    }
  };
  let protocol: Arc<dyn Protocol> =
    Arc::new(ProxyProtocol::new(ProxyResolver::host_mapping(hosts)));
  ok_handle(Box::into_raw(Box::new(WvbProtocol { inner: protocol })))
}

/// # Safety
/// `handle` must be null or a pointer previously returned by a protocol constructor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_protocol_free(handle: *mut WvbProtocol) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}

/// Handle one request. Returns a `WvbResult` whose payload is `{ status, headers }` + the response
/// body; a protocol failure comes back as an error result, so the host can answer it its own way.
///
/// # Safety
/// `handle` must be a valid `WvbProtocol`. `method`/`uri` must be valid C strings; `headers_json`
/// must be null or a JSON object string of `{ name: value }` headers. `body` must be null, or point
/// to `body_len` readable bytes for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_protocol_handle(
  handle: *const WvbProtocol,
  method: *const c_char,
  uri: *const c_char,
  headers_json: *const c_char,
  body: *const u8,
  body_len: usize,
) -> *mut WvbResult {
  let Some(proto) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("protocol");
  };
  let method = unsafe { cstr(method) };
  let uri = unsafe { cstr(uri) };
  let headers_raw = unsafe { cstr(headers_json) };
  let headers: HashMap<String, String> = if headers_raw.is_empty() {
    HashMap::new()
  } else {
    serde_json::from_str(&headers_raw).unwrap_or_default()
  };
  // Copied out before the request runs: the caller only guarantees the bytes for this call.
  let body = if body.is_null() || body_len == 0 {
    Vec::new()
  } else {
    unsafe { std::slice::from_raw_parts(body, body_len) }.to_vec()
  };
  handle_request(proto, &method, &uri, &headers, body)
}

fn handle_request(
  proto: Arc<dyn Protocol>,
  method: &str,
  uri: &str,
  headers: &HashMap<String, String>,
  body: Vec<u8>,
) -> *mut WvbResult {
  // An unparseable method token is a bad request — don't silently coerce it to GET.
  let method = match http::Method::from_bytes(method.to_ascii_uppercase().as_bytes()) {
    Ok(method) => method,
    Err(_) => {
      return err_result(
        ErrorCode::InvalidMethod,
        format!("invalid HTTP method: {method}"),
      );
    }
  };
  let mut builder = http::Request::builder().method(method).uri(uri);
  for (name, value) in headers {
    builder = builder.header(name.as_str(), value.as_str());
  }
  let request = match builder.body(body) {
    Ok(request) => request,
    Err(e) => return err_result(ErrorCode::InvalidRequest, format!("bad request: {e}")),
  };

  match runtime().block_on(async move { proto.handle(request).await }) {
    Ok(response) => {
      let mut headers = serde_json::Map::new();
      for (name, value) in response.headers() {
        headers.insert(
          name.as_str().to_string(),
          serde_json::Value::String(String::from_utf8_lossy(value.as_bytes()).into_owned()),
        );
      }
      let status = response.status().as_u16();
      ok_result(
        serde_json::json!({ "status": status, "headers": headers }),
        response.body().as_ref().to_vec(),
      )
    }
    Err(e) => core_err(e),
  }
}
