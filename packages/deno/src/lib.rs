use base64ct::{Base64, Encoding};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;
use wvb::http;
use wvb::integrity::IntegrityPolicy;
use wvb::protocol::{
  BundleProtocol, HostnameSegment, Protocol, ProxyProtocol, ProxyResolver, UriBundleResolver,
  UriPathResolver,
};
use wvb::remote::{HttpConfig, Remote as CoreRemote};
use wvb::signature::{
  EcdsaSecp256r1Verifier, EcdsaSecp384r1Verifier, Ed25519Verifier, RsaPkcs1V15Verifier,
  RsaPssVerifier, SignatureVerifier,
};
use wvb::source::{self, BundleSource};
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

pub struct WvbResponse {
  status: u16,
  headers_json: CString,
  body: Vec<u8>,
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

/// Create a `BundleSource` (`builtin_dir` read-only, `remote_dir` writable).
///
/// # Safety
/// `builtin_dir` and `remote_dir` must be valid NUL-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_new(
  builtin_dir: *const c_char,
  remote_dir: *const c_char,
) -> *mut WvbSource {
  let builtin = unsafe { cstr(builtin_dir) };
  let remote = unsafe { cstr(remote_dir) };
  let source = BundleSource::builder()
    .builtin_dir(builtin)
    .remote_dir(remote)
    .build();
  Box::into_raw(Box::new(WvbSource {
    inner: Arc::new(source),
  }))
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
/// object with `bundleResolver` and/or `pathResolver` (see `parse_bundle_resolver` /
/// `parse_path_resolver`); an unparsable option returns null rather than silently serving with the
/// default resolvers.
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
    let Ok(options) = serde_json::from_str::<serde_json::Value>(&raw) else {
      return std::ptr::null_mut();
    };
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
  }
  let protocol: Arc<dyn Protocol> = Arc::new(protocol);
  Box::into_raw(Box::new(WvbProtocol { inner: protocol }))
}

/// Create a proxy protocol handler that proxies requests to another server (for dev servers).
///
/// # Safety
/// `hosts_json` must be null or a JSON object string mapping host -> URL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_proxy_protocol_new(hosts_json: *const c_char) -> *mut WvbProtocol {
  let raw = unsafe { cstr(hosts_json) };
  let hosts: HashMap<String, String> = if raw.is_empty() {
    HashMap::new()
  } else {
    serde_json::from_str(&raw).unwrap_or_default()
  };
  let resolver = ProxyResolver::host_mapping(hosts);
  let protocol: Arc<dyn Protocol> = Arc::new(ProxyProtocol::new(resolver));
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

/// Handle one request. Returns an owned `WvbResponse` (read via accessors, then free).
///
/// # Safety
/// `handle` must be a valid `WvbProtocol`. `method`/`uri` must be valid C strings; `headers_json`
/// must be null or a JSON object string of `{ name: value }` headers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_protocol_handle(
  handle: *const WvbProtocol,
  method: *const c_char,
  uri: *const c_char,
  headers_json: *const c_char,
) -> *mut WvbResponse {
  let Some(proto) = (unsafe { handle.as_ref() }).map(|h| h.inner.clone()) else {
    return std::ptr::null_mut();
  };
  let method = unsafe { cstr(method) };
  let uri = unsafe { cstr(uri) };
  let headers_raw = unsafe { cstr(headers_json) };
  let headers: HashMap<String, String> = if headers_raw.is_empty() {
    HashMap::new()
  } else {
    serde_json::from_str(&headers_raw).unwrap_or_default()
  };

  let response = handle_request(proto, &method, &uri, &headers);
  Box::into_raw(Box::new(response))
}

fn handle_request(
  proto: Arc<dyn Protocol>,
  method: &str,
  uri: &str,
  headers: &HashMap<String, String>,
) -> WvbResponse {
  // An unparseable method token is a bad request — don't silently coerce it to GET.
  let method = match http::Method::from_bytes(method.to_ascii_uppercase().as_bytes()) {
    Ok(method) => method,
    Err(_) => return error_response(400, "invalid HTTP method"),
  };
  let mut builder = http::Request::builder().method(method).uri(uri);
  for (name, value) in headers {
    builder = builder.header(name.as_str(), value.as_str());
  }
  let request = match builder.body(Vec::new()) {
    Ok(request) => request,
    Err(e) => return error_response(400, &format!("bad request: {e}")),
  };

  match runtime().block_on(async move { proto.handle(request).await }) {
    Ok(response) => {
      let status = response.status().as_u16();
      let mut map = serde_json::Map::new();
      for (name, value) in response.headers() {
        map.insert(
          name.as_str().to_string(),
          serde_json::Value::String(String::from_utf8_lossy(value.as_bytes()).into_owned()),
        );
      }
      let headers_json =
        CString::new(serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string()))
          .unwrap_or_default();
      WvbResponse {
        status,
        headers_json,
        body: response.body().as_ref().to_vec(),
      }
    }
    Err(e) => error_response(500, &format!("{e}")),
  }
}

fn error_response(status: u16, message: &str) -> WvbResponse {
  WvbResponse {
    status,
    headers_json: CString::new(r#"{"content-type":"text/plain; charset=utf-8"}"#)
      .expect("static header json"),
    body: message.as_bytes().to_vec(),
  }
}

/// # Safety
/// `resp` must be null or a valid `WvbResponse` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_response_status(resp: *const WvbResponse) -> u16 {
  unsafe { resp.as_ref() }.map(|r| r.status).unwrap_or(0)
}

/// Returns a borrowed pointer to the response's headers JSON (valid until `wvb_response_free`).
///
/// # Safety
/// `resp` must be null or a valid `WvbResponse` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_response_headers_json(resp: *const WvbResponse) -> *const c_char {
  match unsafe { resp.as_ref() } {
    Some(r) => r.headers_json.as_ptr(),
    None => std::ptr::null(),
  }
}

/// # Safety
/// `resp` must be null or a valid `WvbResponse` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_response_body_ptr(resp: *const WvbResponse) -> *const u8 {
  match unsafe { resp.as_ref() } {
    Some(r) => r.body.as_ptr(),
    None => std::ptr::null(),
  }
}

/// # Safety
/// `resp` must be null or a valid `WvbResponse` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_response_body_len(resp: *const WvbResponse) -> usize {
  unsafe { resp.as_ref() }.map(|r| r.body.len()).unwrap_or(0)
}

/// # Safety
/// `resp` must be null or a pointer previously returned by `wvb_protocol_handle`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_response_free(resp: *mut WvbResponse) {
  if !resp.is_null() {
    drop(unsafe { Box::from_raw(resp) });
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
fn err_result(code: &str, message: String) -> *mut WvbResult {
  let json = serde_json::json!({ "code": code, "message": message });
  let text = serde_json::to_string(&json).unwrap_or_else(|_| "null".to_string());
  Box::into_raw(Box::new(WvbResult {
    ok: false,
    json: CString::new(text).unwrap_or_default(),
    body: Vec::new(),
  }))
}

/// A `wvb` core error, tagged with its [`wvb::ErrorCode`] as the `core.<code>` wire code.
fn core_err(e: wvb::Error) -> *mut WvbResult {
  err_result(&format!("core.{}", e.code()), e.to_string())
}

/// A handle argument was null, or had already been freed.
fn null_handle_err(what: &str) -> *mut WvbResult {
  err_result("null_handle", format!("{what} handle is null"))
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

/// Build a core `SignatureVerifier` from a `signatureVerifier` JSON object
/// (`{ algorithm, key: { format, data } }`.
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
    if let Some(policy) = value.get("integrityPolicy").and_then(|x| x.as_str()) {
      let policy = match policy {
        "strict" => IntegrityPolicy::Strict,
        "optional" => IntegrityPolicy::Optional,
        "none" => IntegrityPolicy::None,
        _ => IntegrityPolicy::Strict,
      };
      config = config.integrity_policy(policy);
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
