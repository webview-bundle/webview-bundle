use crate::error::ErrorCode;
use crate::result::{
  WvbResult, checksum_result, core_err, data_result, err_result, null_handle_err, ok_handle,
  ok_result, wire_json,
};
use crate::{cstr, owned_bytes, runtime};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::ffi::c_char;
use std::sync::Arc;
use wvb::http;
use wvb::source;
use wvb::{
  AsyncBundleReader, AsyncBundleWriter, AsyncReader, AsyncWriter, Bundle as CoreBundle,
  BundleBuilder as CoreBundleBuilder, BundleDescriptor as CoreBundleDescriptor, BundleEntry,
  BundleReader, BundleWriter, Index, Reader, Writer,
};

/// The `.wvb` bundle format version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Version {
  V1,
}

impl From<wvb::Version> for Version {
  fn from(v: wvb::Version) -> Self {
    match v {
      wvb::Version::V1 => Self::V1,
    }
  }
}

/// A bundle's header: format metadata read from the first bytes of a `.wvb` file.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BundleHeader {
  pub version: Version,
  /// Byte offset where the index section ends (the start of the data section).
  pub index_end_offset: u32,
  /// Size of the index section in bytes.
  pub index_size: u32,
}

impl From<&wvb::Header> for BundleHeader {
  fn from(h: &wvb::Header) -> Self {
    Self {
      version: h.version().into(),
      index_end_offset: h.index_end_offset() as u32,
      index_size: h.index_size(),
    }
  }
}

/// Metadata for a single file in a bundle's index. Sizes are byte counts (`offset`/`len` over the
/// compressed data section; `contentLength` is the original, decompressed size).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexEntry {
  pub offset: u32,
  pub len: u32,
  pub is_empty: bool,
  pub content_type: String,
  pub content_length: u32,
  pub headers: HashMap<String, String>,
}

impl From<&wvb::IndexEntry> for IndexEntry {
  fn from(e: &wvb::IndexEntry) -> Self {
    let mut headers = HashMap::with_capacity(e.headers().len());
    for (name, value) in e.headers() {
      headers.insert(
        name.as_str().to_string(),
        String::from_utf8_lossy(value.as_bytes()).into_owned(),
      );
    }
    Self {
      offset: e.offset() as u32,
      len: e.len() as u32,
      is_empty: e.is_empty(),
      content_type: e.content_type().to_string(),
      content_length: e.content_length() as u32,
      headers,
    }
  }
}

// Bundle authoring & inspection — parity with `@wvb/node`'s Bundle / BundleBuilder /
// BundleDescriptor and read/writeBundle. Each handle owns state a stateless JSON call could not
// reproduce cheaply: a loaded data section (WvbBundle), accumulating builder state
// (WvbBundleBuilder), or a parsed index kept resident for lazy reads (WvbDescriptor /
// WvbLoadedDescriptor). Header/Index/IndexEntry are returned as JSON (see `wire`), matching how the
// binding already returns every other metadata type.

pub(crate) struct WvbBundle {
  pub(crate) inner: Arc<CoreBundle>,
}

pub(crate) struct WvbBundleBuilder {
  inner: std::sync::Mutex<CoreBundleBuilder>,
}

pub(crate) struct WvbDescriptor {
  pub(crate) inner: Arc<CoreBundleDescriptor>,
}

pub(crate) struct WvbLoadedDescriptor {
  pub(crate) inner: Arc<source::LoadedDescriptor>,
}

/// A bundle's index as `{ path: IndexEntry }` JSON — the flattened shape `@wvb/deno` reads.
fn index_json(index: &Index) -> serde_json::Value {
  let entries: HashMap<String, IndexEntry> = index
    .entries()
    .iter()
    .map(|(path, entry)| (path.clone(), IndexEntry::from(entry)))
    .collect();
  wire_json(entries)
}

/// Parse a `{ name: value }` JSON object into an [`http::HeaderMap`]. Returns `None` for a
/// non-object or an invalid header name/value so the caller can reject the entry.
fn parse_header_map(raw: &str) -> Option<http::HeaderMap> {
  let map: HashMap<String, String> = serde_json::from_str(raw).ok()?;
  let mut headers = http::HeaderMap::with_capacity(map.len());
  for (name, value) in map {
    let name = http::HeaderName::from_bytes(name.as_bytes()).ok()?;
    let value = http::HeaderValue::from_str(&value).ok()?;
    headers.insert(name, value);
  }
  Some(headers)
}

/// Parse `{ header?, index?, dataChecksum? }` build options. Unknown or ill-typed keys are
/// rejected (serde's `deny_unknown_fields`), so a misspelled option cannot silently change how the
/// bundle is written; the returned message names the offending key.
fn parse_build_options(raw: &str) -> Result<wvb::BundleBuilderOptions, String> {
  serde_json::from_str::<crate::options::BundleBuilderOptions>(raw)
    .map(Into::into)
    .map_err(|e| e.to_string())
}

/// Read a bundle from a `.wvb` file into an in-memory [`WvbBundle`] handle (freed by
/// `wvb_bundle_free`). On error the reason is preserved in the result.
///
/// # Safety
/// `path` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_read_bundle(path: *const c_char) -> *mut WvbResult {
  let path = unsafe { cstr(path) };
  match runtime().block_on(async move {
    let mut file = tokio::fs::File::open(&path)
      .await
      .map_err(wvb::Error::from)?;
    AsyncReader::<CoreBundle>::read(&mut AsyncBundleReader::new(&mut file)).await
  }) {
    Ok(bundle) => ok_handle(Box::into_raw(Box::new(WvbBundle {
      inner: Arc::new(bundle),
    }))),
    Err(e) => core_err(e),
  }
}

/// Parse a `.wvb` bundle from its bytes (e.g. straight from `wvb_remote_download`'s body) into a
/// [`WvbBundle`] handle.
///
/// # Safety
/// `data` must be null or point to `data_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_read_bundle_from_bytes(
  data: *const u8,
  data_len: usize,
) -> *mut WvbResult {
  let bytes = unsafe { owned_bytes(data, data_len) };
  match Reader::<CoreBundle>::read(&mut BundleReader::new(std::io::Cursor::new(bytes))) {
    Ok(bundle) => ok_handle(Box::into_raw(Box::new(WvbBundle {
      inner: Arc::new(bundle),
    }))),
    Err(e) => core_err(e),
  }
}

/// Write a bundle to a `.wvb` file. The result's json is the number of bytes written.
///
/// # Safety
/// `bundle` must be a valid `WvbBundle`; `path` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_write_bundle(
  bundle: *const WvbBundle,
  path: *const c_char,
) -> *mut WvbResult {
  let Some(bundle) = (unsafe { bundle.as_ref() }).map(|b| b.inner.clone()) else {
    return null_handle_err("bundle");
  };
  let path = unsafe { cstr(path) };
  match runtime().block_on(async move {
    let mut file = tokio::fs::File::create(&path)
      .await
      .map_err(wvb::Error::from)?;
    AsyncWriter::<CoreBundle>::write(&mut AsyncBundleWriter::new(&mut file), &bundle).await
  }) {
    Ok(size) => ok_result(serde_json::json!(size), Vec::new()),
    Err(e) => core_err(e),
  }
}

/// Serialize a bundle to `.wvb` bytes. The result's body is the serialized bytes.
///
/// # Safety
/// `bundle` must be a valid `WvbBundle`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_write_bundle_to_bytes(bundle: *const WvbBundle) -> *mut WvbResult {
  let Some(bundle) = (unsafe { bundle.as_ref() }).map(|b| b.inner.clone()) else {
    return null_handle_err("bundle");
  };
  let mut buf = Vec::new();
  match Writer::<CoreBundle>::write(&mut BundleWriter::new(&mut buf), &bundle) {
    Ok(_) => ok_result(serde_json::Value::Null, buf),
    Err(e) => core_err(e),
  }
}

/// Read decompressed entry bytes for `path` (json `true` + body bytes, or json `null` if absent).
///
/// # Safety
/// `bundle` must be a valid `WvbBundle`; `path` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_bundle_get_data(
  bundle: *const WvbBundle,
  path: *const c_char,
) -> *mut WvbResult {
  let Some(bundle) = (unsafe { bundle.as_ref() }) else {
    return null_handle_err("bundle");
  };
  let path = unsafe { cstr(path) };
  match bundle.inner.get_data(&path) {
    Ok(data) => data_result(data),
    Err(e) => core_err(e),
  }
}

/// Read the stored checksum of the entry data for `path` (json `number` | `null`).
///
/// # Safety
/// `bundle` must be a valid `WvbBundle`; `path` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_bundle_get_data_checksum(
  bundle: *const WvbBundle,
  path: *const c_char,
) -> *mut WvbResult {
  let Some(bundle) = (unsafe { bundle.as_ref() }) else {
    return null_handle_err("bundle");
  };
  let path = unsafe { cstr(path) };
  match bundle.inner.get_data_checksum(&path) {
    Ok(checksum) => checksum_result(checksum),
    Err(e) => core_err(e),
  }
}

/// The bundle header as `{ version, indexEndOffset, indexSize }` JSON.
///
/// # Safety
/// `bundle` must be a valid `WvbBundle`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_bundle_header(bundle: *const WvbBundle) -> *mut WvbResult {
  let Some(bundle) = (unsafe { bundle.as_ref() }) else {
    return null_handle_err("bundle");
  };
  ok_result(
    wire_json(BundleHeader::from(bundle.inner.descriptor().header())),
    Vec::new(),
  )
}

/// The bundle index as `{ path: IndexEntry }` JSON.
///
/// # Safety
/// `bundle` must be a valid `WvbBundle`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_bundle_index(bundle: *const WvbBundle) -> *mut WvbResult {
  let Some(bundle) = (unsafe { bundle.as_ref() }) else {
    return null_handle_err("bundle");
  };
  ok_result(index_json(bundle.inner.descriptor().index()), Vec::new())
}

/// # Safety
/// `handle` must be null or a pointer previously returned as a `WvbBundle`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_bundle_free(handle: *mut WvbBundle) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}

/// Create an empty bundle builder (freed by `wvb_bundle_builder_free`).
#[unsafe(no_mangle)]
pub extern "C" fn wvb_bundle_builder_new() -> *mut WvbBundleBuilder {
  Box::into_raw(Box::new(WvbBundleBuilder {
    inner: std::sync::Mutex::new(CoreBundleBuilder::new()),
  }))
}

/// Add or replace an entry. `content_type` empty falls back to `application/octet-stream`
/// (`@wvb/deno` fills in a type from the path); `headers_json` is null/empty or a `{ name: value }`
/// object. The result's json is `true` when an existing entry was replaced.
///
/// # Safety
/// `builder` must be a valid `WvbBundleBuilder`; `path`/`content_type` valid C strings; `data` null
/// or `data_len` readable bytes; `headers_json` null or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_bundle_builder_insert_entry(
  builder: *const WvbBundleBuilder,
  path: *const c_char,
  data: *const u8,
  data_len: usize,
  content_type: *const c_char,
  headers_json: *const c_char,
) -> *mut WvbResult {
  let Some(builder) = (unsafe { builder.as_ref() }) else {
    return null_handle_err("bundle builder");
  };
  let path = unsafe { cstr(path) };
  let bytes = unsafe { owned_bytes(data, data_len) };
  let content_type = unsafe { cstr(content_type) };
  let content_type = if content_type.is_empty() {
    "application/octet-stream".to_string()
  } else {
    content_type
  };
  let headers_raw = unsafe { cstr(headers_json) };
  let headers = if headers_raw.is_empty() {
    None
  } else {
    match parse_header_map(&headers_raw) {
      Some(headers) => Some(headers),
      None => {
        return err_result(
          ErrorCode::InvalidRequest,
          "invalid entry headers".to_string(),
        );
      }
    }
  };
  let entry = BundleEntry::new(&bytes, content_type, headers);
  let Ok(mut guard) = builder.inner.lock() else {
    return err_result(ErrorCode::Unknown, "bundle builder is poisoned".to_string());
  };
  let replaced = guard.insert_entry(path, entry).is_some();
  ok_result(serde_json::Value::Bool(replaced), Vec::new())
}

/// Remove an entry. The result's json is `true` when an entry existed and was removed.
///
/// # Safety
/// `builder` must be a valid `WvbBundleBuilder`; `path` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_bundle_builder_remove_entry(
  builder: *const WvbBundleBuilder,
  path: *const c_char,
) -> *mut WvbResult {
  let Some(builder) = (unsafe { builder.as_ref() }) else {
    return null_handle_err("bundle builder");
  };
  let path = unsafe { cstr(path) };
  let Ok(mut guard) = builder.inner.lock() else {
    return err_result(ErrorCode::Unknown, "bundle builder is poisoned".to_string());
  };
  let removed = guard.remove_entry(&path).is_some();
  ok_result(serde_json::Value::Bool(removed), Vec::new())
}

/// Whether the builder holds an entry at `path` (json boolean).
///
/// # Safety
/// `builder` must be a valid `WvbBundleBuilder`; `path` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_bundle_builder_contains_entry(
  builder: *const WvbBundleBuilder,
  path: *const c_char,
) -> *mut WvbResult {
  let Some(builder) = (unsafe { builder.as_ref() }) else {
    return null_handle_err("bundle builder");
  };
  let path = unsafe { cstr(path) };
  let Ok(guard) = builder.inner.lock() else {
    return err_result(ErrorCode::Unknown, "bundle builder is poisoned".to_string());
  };
  ok_result(
    serde_json::Value::Bool(guard.contains_path(&path)),
    Vec::new(),
  )
}

/// The builder's entry paths (json string array).
///
/// # Safety
/// `builder` must be a valid `WvbBundleBuilder`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_bundle_builder_entry_paths(
  builder: *const WvbBundleBuilder,
) -> *mut WvbResult {
  let Some(builder) = (unsafe { builder.as_ref() }) else {
    return null_handle_err("bundle builder");
  };
  let Ok(guard) = builder.inner.lock() else {
    return err_result(ErrorCode::Unknown, "bundle builder is poisoned".to_string());
  };
  let paths: Vec<String> = guard.entries().keys().cloned().collect();
  ok_result(serde_json::json!(paths), Vec::new())
}

/// Build a bundle from the current entries. `options_json` is null/empty or `{ header?, index?,
/// dataChecksum? }`. On success the result carries a `WvbBundle` handle.
///
/// # Safety
/// `builder` must be a valid `WvbBundleBuilder`; `options_json` null or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_bundle_builder_build(
  builder: *const WvbBundleBuilder,
  options_json: *const c_char,
) -> *mut WvbResult {
  let Some(builder) = (unsafe { builder.as_ref() }) else {
    return null_handle_err("bundle builder");
  };
  let raw = unsafe { cstr(options_json) };
  let Ok(mut guard) = builder.inner.lock() else {
    return err_result(ErrorCode::Unknown, "bundle builder is poisoned".to_string());
  };
  if !raw.is_empty() {
    match parse_build_options(&raw) {
      Ok(options) => {
        guard.set_options(options);
      }
      Err(message) => return err_result(ErrorCode::InvalidRequest, message),
    }
  }
  match guard.build() {
    Ok(bundle) => ok_handle(Box::into_raw(Box::new(WvbBundle {
      inner: Arc::new(bundle),
    }))),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be null or a pointer previously returned by `wvb_bundle_builder_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_bundle_builder_free(handle: *mut WvbBundleBuilder) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}

/// Read entry bytes for `path` by reopening `filepath`, using the descriptor's cached index. json
/// `true` + body bytes on a hit, json `null` if the path is absent.
///
/// # Safety
/// `descriptor` must be a valid `WvbDescriptor`; `filepath`/`path` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_descriptor_get_data(
  descriptor: *const WvbDescriptor,
  filepath: *const c_char,
  path: *const c_char,
) -> *mut WvbResult {
  let Some(descriptor) = (unsafe { descriptor.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("descriptor");
  };
  let filepath = unsafe { cstr(filepath) };
  let path = unsafe { cstr(path) };
  match runtime().block_on(async move {
    let file = tokio::fs::File::open(&filepath)
      .await
      .map_err(wvb::Error::from)?;
    descriptor.async_get_data(file, &path).await
  }) {
    Ok(data) => data_result(data),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `descriptor` must be a valid `WvbDescriptor`; `filepath`/`path` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_descriptor_get_data_checksum(
  descriptor: *const WvbDescriptor,
  filepath: *const c_char,
  path: *const c_char,
) -> *mut WvbResult {
  let Some(descriptor) = (unsafe { descriptor.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("descriptor");
  };
  let filepath = unsafe { cstr(filepath) };
  let path = unsafe { cstr(path) };
  match runtime().block_on(async move {
    let file = tokio::fs::File::open(&filepath)
      .await
      .map_err(wvb::Error::from)?;
    descriptor.async_get_data_checksum(file, &path).await
  }) {
    Ok(checksum) => checksum_result(checksum),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `descriptor` must be a valid `WvbDescriptor`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_descriptor_header(descriptor: *const WvbDescriptor) -> *mut WvbResult {
  let Some(descriptor) = (unsafe { descriptor.as_ref() }) else {
    return null_handle_err("descriptor");
  };
  ok_result(
    wire_json(BundleHeader::from(descriptor.inner.header())),
    Vec::new(),
  )
}

/// # Safety
/// `descriptor` must be a valid `WvbDescriptor`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_descriptor_index(descriptor: *const WvbDescriptor) -> *mut WvbResult {
  let Some(descriptor) = (unsafe { descriptor.as_ref() }) else {
    return null_handle_err("descriptor");
  };
  ok_result(index_json(descriptor.inner.index()), Vec::new())
}

/// # Safety
/// `handle` must be null or a pointer previously returned as a `WvbDescriptor`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_descriptor_free(handle: *mut WvbDescriptor) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}

/// Read entry bytes for `path` from the loaded descriptor's remembered filepath + read options.
/// json `true` + body bytes on a hit, json `null` if the path is absent.
///
/// # Safety
/// `descriptor` must be a valid `WvbLoadedDescriptor`; `path` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_loaded_descriptor_get_data(
  descriptor: *const WvbLoadedDescriptor,
  path: *const c_char,
) -> *mut WvbResult {
  let Some(descriptor) = (unsafe { descriptor.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("loaded descriptor");
  };
  let path = unsafe { cstr(path) };
  match runtime().block_on(async move { descriptor.get_data(&path).await }) {
    Ok(data) => data_result(data),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `descriptor` must be a valid `WvbLoadedDescriptor`; `path` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_loaded_descriptor_get_data_checksum(
  descriptor: *const WvbLoadedDescriptor,
  path: *const c_char,
) -> *mut WvbResult {
  let Some(descriptor) = (unsafe { descriptor.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("loaded descriptor");
  };
  let path = unsafe { cstr(path) };
  match runtime().block_on(async move { descriptor.get_data_checksum(&path).await }) {
    Ok(checksum) => checksum_result(checksum),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `descriptor` must be a valid `WvbLoadedDescriptor`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_loaded_descriptor_header(
  descriptor: *const WvbLoadedDescriptor,
) -> *mut WvbResult {
  let Some(descriptor) = (unsafe { descriptor.as_ref() }) else {
    return null_handle_err("loaded descriptor");
  };
  ok_result(
    wire_json(BundleHeader::from(descriptor.inner.descriptor().header())),
    Vec::new(),
  )
}

/// # Safety
/// `descriptor` must be a valid `WvbLoadedDescriptor`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_loaded_descriptor_index(
  descriptor: *const WvbLoadedDescriptor,
) -> *mut WvbResult {
  let Some(descriptor) = (unsafe { descriptor.as_ref() }) else {
    return null_handle_err("loaded descriptor");
  };
  ok_result(
    index_json(descriptor.inner.descriptor().index()),
    Vec::new(),
  )
}

/// # Safety
/// `handle` must be null or a pointer previously returned as a `WvbLoadedDescriptor`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_loaded_descriptor_free(handle: *mut WvbLoadedDescriptor) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}
