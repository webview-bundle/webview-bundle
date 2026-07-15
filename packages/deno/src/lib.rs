mod error;

use base64ct::{Base64, Encoding};
use error::ErrorCode;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;
use wvb::http;
use wvb::integrity::IntegrityPolicy;
use wvb::protocol::{
  BundleProtocol, BundleProtocolOptions, HostnameSegment, Protocol, ProxyProtocol, ProxyResolver,
  UriBundleResolver, UriPathResolver,
};
use wvb::remote::{HttpConfig, Remote as CoreRemote};
use wvb::signature::{
  EcdsaSecp256r1Verifier, EcdsaSecp384r1Verifier, Ed25519Verifier, RsaPkcs1V15Verifier,
  RsaPssVerifier, SignatureVerifier,
};
use wvb::source::{
  self, BundleSource, BundleSourceIntegrityOptions, BundleSourceOptions,
  BundleSourceSignatureOptions, BundleSourceVerifyMode,
};
use wvb::updater::{Updater as CoreUpdater, UpdaterConfig};

fn runtime() -> &'static Runtime {
  static RT: OnceLock<Runtime> = OnceLock::new();
  RT.get_or_init(|| {
    tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .build()
      .expect("failed to build tokio runtime")
  })
}

pub struct WvbSource {
  inner: Arc<BundleSource>,
}

pub struct WvbProtocol {
  inner: Arc<dyn Protocol>,
}

/// # Safety
/// `ptr` must be null or a valid NUL-terminated C string.
unsafe fn cstr(ptr: *const c_char) -> String {
  if ptr.is_null() {
    return String::new();
  }
  unsafe { CStr::from_ptr(ptr) }
    .to_string_lossy()
    .into_owned()
}

/// Create a `BundleSource` (`builtin_dir` read-only, `remote_dir` writable) with the default
/// options (remote bundles checked against their manifest integrity on load, under the optional
/// policy; no signature verification; entry data checksums verified on read, with seed `0`).
///
/// # Safety
/// `builtin_dir` and `remote_dir` must be valid NUL-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_new(
  builtin_dir: *const c_char,
  remote_dir: *const c_char,
) -> *mut WvbSource {
  unsafe { wvb_source_new_with_options(builtin_dir, remote_dir, std::ptr::null()) }
}

/// Create a `BundleSource` with options. `options_json` is null/empty or a JSON object with
/// `integrity`, `signature`, `verifyDataChecksum` and/or `dataChecksumSeed`; an unparsable option
/// returns null rather than silently reading bundles unverified.
///
/// # Safety
/// `builtin_dir`/`remote_dir` must be valid NUL-terminated C strings; `options_json` must be null
/// or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_new_with_options(
  builtin_dir: *const c_char,
  remote_dir: *const c_char,
  options_json: *const c_char,
) -> *mut WvbSource {
  let builtin = unsafe { cstr(builtin_dir) };
  let remote = unsafe { cstr(remote_dir) };
  let raw = unsafe { cstr(options_json) };
  let mut builder = BundleSource::builder()
    .builtin_dir(builtin)
    .remote_dir(remote);
  if !raw.is_empty() {
    // A scalar or array would read as "no options given" below — fail closed instead.
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
      return std::ptr::null_mut();
    };
    if !value.is_object() {
      return std::ptr::null_mut();
    }
    let Some(options) = parse_source_options(&value) else {
      return std::ptr::null_mut();
    };
    builder = builder.options(options);
  }
  Box::into_raw(Box::new(WvbSource {
    inner: Arc::new(builder.build()),
  }))
}

/// Parse a source `options` JSON object (camelCase, mirroring `BundleSourceOptions` in the other
/// bindings). Returns `None` for an unknown or ill-typed value, so the caller can fail closed.
fn parse_source_options(value: &serde_json::Value) -> Option<BundleSourceOptions> {
  let mut options = BundleSourceOptions::default();
  match value.get("integrity") {
    None | Some(serde_json::Value::Null) => {}
    Some(x) => {
      if !x.is_object() {
        return None;
      }
      let mut integrity = BundleSourceIntegrityOptions::default();
      match x.get("policy") {
        None | Some(serde_json::Value::Null) => {}
        Some(p) => integrity = integrity.policy(parse_integrity_policy(p.as_str()?)?),
      }
      match x.get("checkMode") {
        None | Some(serde_json::Value::Null) => {}
        Some(m) => integrity = integrity.check_mode(parse_verify_mode(m.as_str()?)?),
      }
      options = options.integrity(integrity);
    }
  }
  // A present-but-unbuildable signature verifier fails closed (null source) rather than silently
  // reading bundles unverified.
  match value.get("signature") {
    None | Some(serde_json::Value::Null) => {}
    Some(x) => {
      if !x.is_object() {
        return None;
      }
      let mut signature = BundleSourceSignatureOptions::default();
      match x.get("verify") {
        None | Some(serde_json::Value::Null) => {}
        Some(sv) => signature = signature.verify(build_signature_verifier(sv)?),
      }
      match x.get("verifyMode") {
        None | Some(serde_json::Value::Null) => {}
        Some(m) => signature = signature.verify_mode(parse_verify_mode(m.as_str()?)?),
      }
      options = options.signature(signature);
    }
  }
  // Only the keys the caller actually gave are overridden: `BundleSourceOptions` verifies data
  // checksums by default, and replacing its `DataReadOptions` wholesale would turn that back off.
  match value.get("verifyDataChecksum") {
    None | Some(serde_json::Value::Null) => {}
    Some(x) => options = options.verify_data_checksum(x.as_bool()?),
  }
  match value.get("dataChecksumSeed") {
    None | Some(serde_json::Value::Null) => {}
    Some(x) => options = options.data_checksum_seed(u32::try_from(x.as_u64()?).ok()?),
  }
  Some(options)
}

/// `integrity.policy`/`integrityPolicy` string mapping shared by the source and the updater.
/// Returns `None` for an unknown value, so the caller can fail closed rather than pick a default.
fn parse_integrity_policy(policy: &str) -> Option<IntegrityPolicy> {
  match policy {
    "strict" => Some(IntegrityPolicy::Strict),
    "optional" => Some(IntegrityPolicy::Optional),
    "off" => Some(IntegrityPolicy::Off),
    _ => None,
  }
}

/// `integrity.checkMode`/`signature.verifyMode` string mapping.
fn parse_verify_mode(mode: &str) -> Option<BundleSourceVerifyMode> {
  match mode {
    "all" => Some(BundleSourceVerifyMode::All),
    "onlyRemote" => Some(BundleSourceVerifyMode::OnlyRemote),
    _ => None,
  }
}

/// Read the same two data-checksum keys for a protocol, which carries its own options type but the
/// same defaults: verification is ON with seed `0` unless the key says otherwise.
fn parse_protocol_data_options(value: &serde_json::Value) -> Option<BundleProtocolOptions> {
  let mut options = BundleProtocolOptions::new();
  match value.get("verifyDataChecksum") {
    None | Some(serde_json::Value::Null) => {}
    Some(x) => options = options.verify_data_checksum(x.as_bool()?),
  }
  match value.get("dataChecksumSeed") {
    None | Some(serde_json::Value::Null) => {}
    Some(x) => options = options.data_checksum_seed(u32::try_from(x.as_u64()?).ok()?),
  }
  Some(options)
}

/// # Safety
/// `handle` must be null or a pointer previously returned by `wvb_source_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_free(handle: *mut WvbSource) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
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
          "first" => HostnameSegment::First,
          "full" => HostnameSegment::Full,
          "stripSuffix" => HostnameSegment::StripSuffix,
          _ => return None,
        }),
        Some(serde_json::Value::Number(n)) => Some(HostnameSegment::Nth(n.as_u64()? as usize)),
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
/// object with `bundleResolver`, `pathResolver`, `verifyDataChecksum` and/or `dataChecksumSeed`;
/// an unparsable option returns null rather than silently serving with the default resolvers.
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
        Some(resolver) => protocol = protocol.with_bundle_resolver(resolver),
        None => return std::ptr::null_mut(),
      },
    }
    match options.get("pathResolver") {
      None | Some(serde_json::Value::Null) => {}
      Some(value) => match parse_path_resolver(value) {
        Some(resolver) => protocol = protocol.with_path_resolver(resolver),
        None => return std::ptr::null_mut(),
      },
    }
    match parse_protocol_data_options(&options) {
      Some(data) => protocol = protocol.with_options(data),
      None => return std::ptr::null_mut(),
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

pub struct WvbRemote {
  inner: Arc<CoreRemote>,
}

pub struct WvbUpdater {
  inner: CoreUpdater,
}

/// Result of a data-API call: `json` (payload on success / `{ code, message }` on error) +
/// optional `body` bytes.
pub struct WvbResult {
  ok: bool,
  json: CString,
  body: Vec<u8>,
}

fn ok_result(json: serde_json::Value, body: Vec<u8>) -> *mut WvbResult {
  let text = serde_json::to_string(&json).unwrap_or_else(|_| "null".to_string());
  Box::into_raw(Box::new(WvbResult {
    ok: true,
    json: CString::new(text).unwrap_or_default(),
    body,
  }))
}

/// An error result carrying the stable code alongside the message, so `@wvb/deno` can rebuild a
/// `WebviewBundleError` with the same code the other bindings use.
fn err_result(code: ErrorCode, message: String) -> *mut WvbResult {
  let json = serde_json::json!({ "code": code.as_str(), "message": message });
  let text = serde_json::to_string(&json).unwrap_or_else(|_| "null".to_string());
  Box::into_raw(Box::new(WvbResult {
    ok: false,
    json: CString::new(text).unwrap_or_default(),
    body: Vec::new(),
  }))
}

/// A `wvb` core error, tagged with its [`wvb::ErrorCode`] as the `core.<code>` wire code.
fn core_err(e: wvb::Error) -> *mut WvbResult {
  err_result(e.code().into(), e.to_string())
}

/// A handle argument was null, or had already been freed.
fn null_handle_err(what: &str) -> *mut WvbResult {
  err_result(ErrorCode::NullHandle, format!("{what} handle is null"))
}

fn list_info_json(info: &wvb::remote::ListRemoteBundleInfo) -> serde_json::Value {
  serde_json::json!({ "name": info.name, "version": info.version })
}

fn remote_info_json(info: &wvb::remote::RemoteBundleInfo) -> serde_json::Value {
  serde_json::json!({
    "name": info.name,
    "version": info.version,
    "etag": info.etag,
    "integrity": info.integrity,
    "signature": info.signature,
    "lastModified": info.last_modified,
  })
}

fn update_info_json(info: &wvb::updater::BundleUpdateInfo) -> serde_json::Value {
  serde_json::json!({
    "name": info.name,
    "version": info.version,
    "localVersion": info.local_version,
    "isAvailable": info.is_available,
    "etag": info.etag,
    "integrity": info.integrity,
    "signature": info.signature,
    "lastModified": info.last_modified,
  })
}

fn source_kind_str(kind: &source::BundleSourceKind) -> &'static str {
  match kind {
    source::BundleSourceKind::Builtin => "builtin",
    source::BundleSourceKind::Remote => "remote",
  }
}

fn manifest_metadata_json(m: &source::BundleManifestMetadata) -> serde_json::Value {
  serde_json::json!({
    "etag": m.etag,
    "integrity": m.integrity,
    "signature": m.signature,
    "lastModified": m.last_modified,
  })
}

fn source_version_json(v: &source::BundleSourceVersion) -> serde_json::Value {
  serde_json::json!({ "type": source_kind_str(&v.kind), "version": v.version })
}

fn list_bundle_item_json(it: &source::ListBundleItem) -> serde_json::Value {
  serde_json::json!({
    "type": source_kind_str(&it.kind),
    "name": it.item.name,
    "version": it.item.version,
    "current": it.item.current,
    "metadata": manifest_metadata_json(&it.item.metadata),
  })
}

/// Parse a JSON object of HTTP client options into an `HttpConfig`.
fn parse_http_config(raw: &str) -> Option<HttpConfig> {
  let value: serde_json::Value = serde_json::from_str(raw).ok()?;
  let mut config = HttpConfig::new();
  if let Some(x) = value.get("userAgent").and_then(|x| x.as_str()) {
    config = config.user_agent(x.to_string());
  }
  if let Some(x) = value.get("timeout").and_then(|x| x.as_u64()) {
    config = config.timeout(x);
  }
  if let Some(x) = value.get("readTimeout").and_then(|x| x.as_u64()) {
    config = config.read_timeout(x);
  }
  if let Some(x) = value.get("connectTimeout").and_then(|x| x.as_u64()) {
    config = config.connect_timeout(x);
  }
  if let Some(x) = value.get("poolIdleTimeout").and_then(|x| x.as_u64()) {
    config = config.pool_idle_timeout(x);
  }
  if let Some(x) = value.get("poolMaxIdlePerHost").and_then(|x| x.as_u64()) {
    config = config.pool_max_idle_per_host(x as usize);
  }
  if let Some(x) = value.get("referer").and_then(|x| x.as_bool()) {
    config = config.referer(x);
  }
  if let Some(x) = value.get("tcpNodelay").and_then(|x| x.as_bool()) {
    config = config.tcp_nodelay(x);
  }
  if let Some(x) = value.get("hickoryDns").and_then(|x| x.as_bool()) {
    config = config.hickory_dns(x);
  }
  Some(config)
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
  // parse_http_config returns None for an empty / unparsable string, so no separate guard is needed.
  let http_raw = unsafe { cstr(http_json) };
  if let Some(config) = parse_http_config(&http_raw) {
    builder = builder.http(config);
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
  match runtime().block_on(async move {
    let channel = (!channel.is_empty()).then_some(&channel);
    remote.list_bundles(channel).await
  }) {
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
    let channel = (!channel.is_empty()).then_some(&channel);
    remote.get_current_info(&name, channel).await
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

/// Build a core `SignatureVerifier` from a verifier JSON object (the source's `signature.verify`
/// or the updater's `signatureVerifier`): `{ algorithm, key: { format, data } }`.
///
/// For the PEM key formats `data` is the PEM text; for the binary formats (`spkiDer`/`pkcs1Der`
/// /`sec1`/`raw`) it is standard base64. Returns `None` on any parse, base64-decode, unsupported
/// algorithm/format combination, or key-construction failure, so the caller can fail closed.
fn build_signature_verifier(sv: &serde_json::Value) -> Option<SignatureVerifier> {
  let algorithm = sv.get("algorithm")?.as_str()?;
  let key = sv.get("key")?;
  let format = key.get("format")?.as_str()?;
  let data = key.get("data")?.as_str()?;
  let bytes = || Base64::decode_vec(data).ok();
  let verifier = match (algorithm, format) {
    ("ecdsaSecp256R1", "sec1") => SignatureVerifier::EcdsaSecp256r1(Arc::new(
      EcdsaSecp256r1Verifier::from_sec1_bytes(&bytes()?).ok()?,
    )),
    ("ecdsaSecp256R1", "spkiDer") => SignatureVerifier::EcdsaSecp256r1(Arc::new(
      EcdsaSecp256r1Verifier::from_public_key_der(&bytes()?).ok()?,
    )),
    ("ecdsaSecp256R1", "spkiPem") => SignatureVerifier::EcdsaSecp256r1(Arc::new(
      EcdsaSecp256r1Verifier::from_public_key_pem(data).ok()?,
    )),
    ("ecdsaSecp384R1", "sec1") => SignatureVerifier::EcdsaSecp384r1(Arc::new(
      EcdsaSecp384r1Verifier::from_sec1_bytes(&bytes()?).ok()?,
    )),
    ("ecdsaSecp384R1", "spkiDer") => SignatureVerifier::EcdsaSecp384r1(Arc::new(
      EcdsaSecp384r1Verifier::from_public_key_der(&bytes()?).ok()?,
    )),
    ("ecdsaSecp384R1", "spkiPem") => SignatureVerifier::EcdsaSecp384r1(Arc::new(
      EcdsaSecp384r1Verifier::from_public_key_pem(data).ok()?,
    )),
    ("ed25519", "spkiDer") => SignatureVerifier::Ed25519(Arc::new(
      Ed25519Verifier::from_public_key_der(&bytes()?).ok()?,
    )),
    ("ed25519", "spkiPem") => {
      SignatureVerifier::Ed25519(Arc::new(Ed25519Verifier::from_public_key_pem(data).ok()?))
    }
    ("ed25519", "raw") => {
      let raw = bytes()?;
      // Ed25519 raw keys must be exactly 32 bytes; reject anything else (fail closed).
      let arr: [u8; 32] = raw.as_slice().try_into().ok()?;
      SignatureVerifier::Ed25519(Arc::new(Ed25519Verifier::from_public_key_bytes(&arr).ok()?))
    }
    ("rsaPkcs1V15", "pkcs1Der") => SignatureVerifier::RsaPkcs1V15(Arc::new(
      RsaPkcs1V15Verifier::from_pkcs1_der(&bytes()?).ok()?,
    )),
    ("rsaPkcs1V15", "pkcs1Pem") => {
      SignatureVerifier::RsaPkcs1V15(Arc::new(RsaPkcs1V15Verifier::from_pkcs1_pem(data).ok()?))
    }
    ("rsaPkcs1V15", "spkiDer") => SignatureVerifier::RsaPkcs1V15(Arc::new(
      RsaPkcs1V15Verifier::from_public_key_der(&bytes()?).ok()?,
    )),
    ("rsaPkcs1V15", "spkiPem") => SignatureVerifier::RsaPkcs1V15(Arc::new(
      RsaPkcs1V15Verifier::from_public_key_pem(data).ok()?,
    )),
    ("rsaPss", "pkcs1Der") => {
      SignatureVerifier::RsaPss(Arc::new(RsaPssVerifier::from_pkcs1_der(&bytes()?).ok()?))
    }
    ("rsaPss", "pkcs1Pem") => {
      SignatureVerifier::RsaPss(Arc::new(RsaPssVerifier::from_pkcs1_pem(data).ok()?))
    }
    ("rsaPss", "spkiDer") => SignatureVerifier::RsaPss(Arc::new(
      RsaPssVerifier::from_public_key_der(&bytes()?).ok()?,
    )),
    ("rsaPss", "spkiPem") => {
      SignatureVerifier::RsaPss(Arc::new(RsaPssVerifier::from_public_key_pem(data).ok()?))
    }
    _ => return None,
  };
  Some(verifier)
}

/// # Safety
/// `source`/`remote` must be valid handles; `options_json` null or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_new(
  source: *const WvbSource,
  remote: *const WvbRemote,
  options_json: *const c_char,
) -> *mut WvbUpdater {
  let (Some(source), Some(remote)) = (unsafe { source.as_ref() }, unsafe { remote.as_ref() })
  else {
    return std::ptr::null_mut();
  };
  let raw = unsafe { cstr(options_json) };
  let config = if raw.is_empty() {
    None
  } else {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
      return std::ptr::null_mut();
    };
    let mut config = UpdaterConfig::default();
    if let Some(channel) = value.get("channel").and_then(|x| x.as_str()) {
      config = config.channel(channel.to_string());
    }
    // An unknown policy value fails closed (null updater) rather than being silently ignored.
    match value.get("integrityPolicy") {
      None | Some(serde_json::Value::Null) => {}
      Some(x) => match x.as_str().and_then(parse_integrity_policy) {
        Some(policy) => config = config.integrity_policy(policy),
        None => return std::ptr::null_mut(),
      },
    }
    // A present-but-unbuildable signatureVerifier fails closed (null updater) rather than silently
    // serving updates unverified.
    match value.get("signatureVerifier") {
      None | Some(serde_json::Value::Null) => {}
      Some(sv) => match build_signature_verifier(sv) {
        Some(verifier) => config = config.signature_verifier(verifier),
        None => return std::ptr::null_mut(),
      },
    }
    Some(config)
  };
  let updater = CoreUpdater::new(source.inner.clone(), remote.inner.clone(), config);
  Box::into_raw(Box::new(WvbUpdater { inner: updater }))
}

/// # Safety
/// `handle` must be null or a pointer previously returned by `wvb_updater_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_free(handle: *mut WvbUpdater) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}

/// # Safety
/// `handle` must be a valid `WvbUpdater`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_list_remotes(handle: *const WvbUpdater) -> *mut WvbResult {
  let Some(updater) = (unsafe { handle.as_ref() }) else {
    return null_handle_err("updater");
  };
  match runtime().block_on(updater.inner.list_remotes()) {
    Ok(list) => ok_result(
      serde_json::Value::Array(list.iter().map(list_info_json).collect()),
      Vec::new(),
    ),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbUpdater`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_get_update(
  handle: *const WvbUpdater,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let Some(updater) = (unsafe { handle.as_ref() }) else {
    return null_handle_err("updater");
  };
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(updater.inner.get_update(&name)) {
    Ok(info) => ok_result(update_info_json(&info), Vec::new()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbUpdater`; `bundle_name` a valid C string; `version` "" = latest.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_download(
  handle: *const WvbUpdater,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let Some(updater) = (unsafe { handle.as_ref() }) else {
    return null_handle_err("updater");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  let version = (!version.is_empty()).then_some(version);
  match runtime().block_on(updater.inner.download(name, version)) {
    Ok(info) => ok_result(remote_info_json(&info), Vec::new()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbUpdater`; `bundle_name`/`version` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_install(
  handle: *const WvbUpdater,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let Some(updater) = (unsafe { handle.as_ref() }) else {
    return null_handle_err("updater");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  match runtime().block_on(updater.inner.install(name, version)) {
    Ok(()) => ok_result(serde_json::Value::Null, Vec::new()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_list_bundles(handle: *const WvbSource) -> *mut WvbResult {
  let Some(source) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  match runtime().block_on(async move { source.list_bundles().await }) {
    Ok(items) => ok_result(
      serde_json::Value::Array(items.iter().map(list_bundle_item_json).collect()),
      Vec::new(),
    ),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_load_version(
  handle: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(async move { source.load_version(&name).await }) {
    Ok(Some(v)) => ok_result(source_version_json(&v), Vec::new()),
    Ok(None) => ok_result(serde_json::Value::Null, Vec::new()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_update_version(
  handle: *const WvbSource,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  match runtime().block_on(async move { source.update_remote_version(&name, &version).await }) {
    Ok(()) => ok_result(serde_json::Value::Null, Vec::new()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_resolve_filepath(
  handle: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(async move { source.resolve_filepath(&name).await }) {
    Ok(path) => ok_result(
      serde_json::Value::String(path.to_string_lossy().into_owned()),
      Vec::new(),
    ),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_get_builtin_filepath(
  handle: *const WvbSource,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { handle.as_ref() }) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  match source.inner.get_builtin_bundle_filepath(&name, &version) {
    Ok(path) => ok_result(
      serde_json::Value::String(path.to_string_lossy().into_owned()),
      Vec::new(),
    ),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_get_remote_filepath(
  handle: *const WvbSource,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { handle.as_ref() }) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  match source.inner.get_remote_bundle_filepath(&name, &version) {
    Ok(path) => ok_result(
      serde_json::Value::String(path.to_string_lossy().into_owned()),
      Vec::new(),
    ),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_load_builtin_metadata(
  handle: *const WvbSource,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  match runtime().block_on(async move { source.load_builtin_metadata(&name, &version).await }) {
    Ok(Some(m)) => ok_result(manifest_metadata_json(&m), Vec::new()),
    Ok(None) => ok_result(serde_json::Value::Null, Vec::new()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_load_remote_metadata(
  handle: *const WvbSource,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  match runtime().block_on(async move { source.load_remote_metadata(&name, &version).await }) {
    Ok(Some(m)) => ok_result(manifest_metadata_json(&m), Vec::new()),
    Ok(None) => ok_result(serde_json::Value::Null, Vec::new()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_unload_descriptor(
  handle: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { handle.as_ref() }) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  ok_result(
    serde_json::Value::Bool(source.inner.unload_descriptor(&name)),
    Vec::new(),
  )
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_remove_remote_bundle(
  handle: *const WvbSource,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  match runtime().block_on(async move { source.remove_remote_bundle(&name, &version).await }) {
    Ok(removed) => ok_result(serde_json::Value::Bool(removed), Vec::new()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_remote_retained_versions(
  handle: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(async move { source.remote_retained_versions(&name).await }) {
    Ok(versions) => ok_result(serde_json::json!(versions), Vec::new()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_prune_remote_bundles(
  handle: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(async move { source.prune_remote_bundles(&name).await }) {
    Ok(removed) => ok_result(serde_json::json!(removed), Vec::new()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `result` must be null or a valid `WvbResult` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_result_ok(result: *const WvbResult) -> u8 {
  match unsafe { result.as_ref() } {
    Some(r) if r.ok => 1,
    _ => 0,
  }
}

/// Borrowed pointer to the result's JSON/message string (valid until `wvb_result_free`).
///
/// # Safety
/// `result` must be null or a valid `WvbResult` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_result_json(result: *const WvbResult) -> *const c_char {
  match unsafe { result.as_ref() } {
    Some(r) => r.json.as_ptr(),
    None => std::ptr::null(),
  }
}

/// # Safety
/// `result` must be null or a valid `WvbResult` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_result_body_ptr(result: *const WvbResult) -> *const u8 {
  match unsafe { result.as_ref() } {
    Some(r) => r.body.as_ptr(),
    None => std::ptr::null(),
  }
}

/// # Safety
/// `result` must be null or a valid `WvbResult` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_result_body_len(result: *const WvbResult) -> usize {
  unsafe { result.as_ref() }
    .map(|r| r.body.len())
    .unwrap_or(0)
}

/// # Safety
/// `result` must be null or a pointer previously returned by a Remote/Updater call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_result_free(result: *mut WvbResult) {
  if !result.is_null() {
    drop(unsafe { Box::from_raw(result) });
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parsed(raw: &str) -> Option<BundleSourceOptions> {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    parse_source_options(&value)
  }

  /// The data options a source parsed from `raw` ends up with.
  fn data_options(raw: &str) -> Option<wvb::DataReadOptions> {
    Some(
      BundleSource::builder()
        .options(parsed(raw)?)
        .build()
        .data_read_options(),
    )
  }

  #[test]
  fn source_verifies_data_checksums_by_default() {
    let data = data_options("{}").unwrap();
    assert!(data.checksum.verify);
    assert_eq!(data.checksum.seed, 0);

    // Overriding the seed must not turn verification back off.
    let data = data_options(r#"{"dataChecksumSeed":7}"#).unwrap();
    assert!(data.checksum.verify);
    assert_eq!(data.checksum.seed, 7);

    // Nor must an unrelated option.
    let data = data_options(r#"{"integrity":{"checkMode":"onlyRemote"}}"#).unwrap();
    assert!(data.checksum.verify);
  }

  #[test]
  fn source_data_checksum_can_be_turned_off() {
    let data = data_options(r#"{"verifyDataChecksum":false}"#).unwrap();
    assert!(!data.checksum.verify);
  }

  #[test]
  fn source_options_accept_the_nested_verification_shape() {
    assert!(parsed(r#"{"integrity":{"policy":"strict","checkMode":"all"}}"#).is_some());
    assert!(parsed(r#"{"integrity":{"policy":"off"}}"#).is_some());
    assert!(parsed(r#"{"signature":{"verifyMode":"all"}}"#).is_some());
  }

  #[test]
  fn source_options_fail_closed_on_a_bad_value() {
    assert!(parsed(r#"{"verifyDataChecksum":"yes"}"#).is_none());
    assert!(parsed(r#"{"dataChecksumSeed":-1}"#).is_none());
    assert!(parsed(r#"{"dataChecksumSeed":4294967296}"#).is_none());
    assert!(parsed(r#"{"integrity":"strict"}"#).is_none());
    assert!(parsed(r#"{"integrity":{"policy":"none"}}"#).is_none());
    assert!(parsed(r#"{"integrity":{"checkMode":"remote"}}"#).is_none());
    assert!(parsed(r#"{"signature":{"verifyMode":"sometimes"}}"#).is_none());
  }

  #[test]
  fn integrity_policy_fails_closed_on_an_unknown_value() {
    assert!(matches!(
      parse_integrity_policy("strict"),
      Some(IntegrityPolicy::Strict)
    ));
    assert!(matches!(
      parse_integrity_policy("optional"),
      Some(IntegrityPolicy::Optional)
    ));
    assert!(matches!(
      parse_integrity_policy("off"),
      Some(IntegrityPolicy::Off)
    ));
    // The old spelling of 'off' must not silently map to a different policy.
    assert!(parse_integrity_policy("none").is_none());
  }
}
