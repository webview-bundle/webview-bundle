#![allow(dead_code)]

use crate::error::ErrorCode;
use crate::result::{WvbResult, core_err, err_result, null_handle_err, ok_result};
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
  UriBundleResolver, UriPathResolver,
};

/// Which hostname segment is used as the bundle name.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum HostnameSegment {
  First,
  Full,
  StripSuffix,
}

/// How the file path in the bundle is resolved from the request uri.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PathResolver {
  Exact,
  DirectoryIndex,
  HtmlExtension,
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
}

pub struct WvbProtocol {
  inner: Arc<dyn Protocol>,
}

/// Parse a `bundleResolver` JSON object (camelCase, mirroring `@wvb/node`'s
/// `BundleResolverOptions`): `{ "type": "hostname", "segment"?: "first" | "full" | "stripSuffix" |
/// number, "allowWvbSuffixOnly"?: boolean }` or `{ "type": "pathname", "segmentIndex"?: number }`.
/// Returns `None` for an unknown discriminant or value, so the caller can fail closed.
fn parse_bundle_resolver(value: &serde_json::Value) -> Option<UriBundleResolver> {
  match value.get("type")?.as_str()? {
    "hostname" => {
      let segment = match value.get("segment") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(match s.as_str() {
          "first" => CoreHostnameSegment::First,
          "full" => CoreHostnameSegment::Full,
          "stripSuffix" => CoreHostnameSegment::StripSuffix,
          _ => return None,
        }),
        Some(serde_json::Value::Number(n)) => Some(CoreHostnameSegment::Nth(n.as_u64()? as usize)),
        Some(_) => return None,
      };
      let allow_wvb_suffix_only = match value.get("allowWvbSuffixOnly") {
        None | Some(serde_json::Value::Null) => None,
        Some(x) => Some(x.as_bool()?),
      };
      Some(UriBundleResolver::hostname(segment, allow_wvb_suffix_only))
    }
    "pathname" => {
      let segment_index = match value.get("segmentIndex") {
        None | Some(serde_json::Value::Null) => None,
        Some(x) => Some(x.as_u64()? as usize),
      };
      Some(UriBundleResolver::pathname(segment_index))
    }
    _ => None,
  }
}

/// Parse a `pathResolver` value: `"exact" | "directoryIndex" | "htmlExtension"`.
fn parse_path_resolver(value: &serde_json::Value) -> Option<UriPathResolver> {
  match value.as_str()? {
    "exact" => Some(UriPathResolver::exact()),
    "directoryIndex" => Some(UriPathResolver::directory_index()),
    "htmlExtension" => Some(UriPathResolver::html_extension()),
    _ => None,
  }
}

/// Create a bundle protocol handler serving from `source`. `options_json` is null/empty or a JSON
/// object with `bundleResolver` and/or `pathResolver`; an unparsable option returns null rather than
/// silently serving with the default resolvers. Entries are served with the read options the
/// `source` was configured with.
///
/// # Safety
/// `source` must be a valid pointer returned by `wvb_source_new`; `options_json` must be null or a
/// valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_bundle_protocol_new(
  source: *const WvbSource,
  options_json: *const c_char,
) -> *mut WvbProtocol {
  let Some(source) = (unsafe { source.as_ref() }) else {
    return std::ptr::null_mut();
  };
  let mut protocol = BundleProtocol::new(source.inner.clone());
  let raw = unsafe { cstr(options_json) };
  if !raw.is_empty() {
    // A scalar or array would read as "no options given" below — fail closed instead.
    let Ok(options) = serde_json::from_str::<serde_json::Value>(&raw) else {
      return std::ptr::null_mut();
    };
    if !options.is_object() {
      return std::ptr::null_mut();
    }
    match options.get("bundleResolver") {
      None | Some(serde_json::Value::Null) => {}
      Some(value) => match parse_bundle_resolver(value) {
        Some(resolver) => protocol = protocol.set_bundle_resolver(resolver),
        None => return std::ptr::null_mut(),
      },
    }
    match options.get("pathResolver") {
      None | Some(serde_json::Value::Null) => {}
      Some(value) => match parse_path_resolver(value) {
        Some(resolver) => protocol = protocol.set_path_resolver(resolver),
        None => return std::ptr::null_mut(),
      },
    }
  }
  let protocol: Arc<dyn Protocol> = Arc::new(protocol);
  Box::into_raw(Box::new(WvbProtocol { inner: protocol }))
}

/// Create a proxy protocol handler that proxies requests to another server (for dev servers).
/// An unparsable host mapping returns null rather than silently proxying nothing.
///
/// # Safety
/// `hosts_json` must be null or a JSON object string mapping host -> URL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_proxy_protocol_new(hosts_json: *const c_char) -> *mut WvbProtocol {
  let raw = unsafe { cstr(hosts_json) };
  let hosts: HashMap<String, String> = if raw.is_empty() {
    HashMap::new()
  } else {
    match serde_json::from_str(&raw) {
      Ok(hosts) => hosts,
      Err(_) => return std::ptr::null_mut(),
    }
  };
  let protocol: Arc<dyn Protocol> =
    Arc::new(ProxyProtocol::new(ProxyResolver::host_mapping(hosts)));
  Box::into_raw(Box::new(WvbProtocol { inner: protocol }))
}

/// # Safety
/// `handle` must be null or a pointer previously returned by a `*_protocol_new`.
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
