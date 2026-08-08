#![allow(dead_code)]

use crate::bundle::{WvbBundle, WvbDescriptor, WvbLoadedDescriptor};
use crate::error::ErrorCode;
use crate::integrity::parse_integrity_policy;
use crate::result::{
  WvbResult, core_err, err_result, null_handle_err, ok_handle, ok_result, wire_json,
};
use crate::signature::build_signature_verifier;
use crate::{cstr, owned_bytes, runtime};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::ffi::c_char;
use std::sync::Arc;
use wvb::source::{
  self, BundleSource, BundleSourceIntegrityOptions, BundleSourceOptions,
  BundleSourceSignatureOptions, BundleSourceVerifyMode as CoreBundleSourceVerifyMode,
};

/// Which bundles a load-time verification applies to.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum BundleSourceVerifyMode {
  All,
  OnlyRemote,
}

/// The type of bundle source: builtin or remote. (`BundleSourceKind` in core; the deno binding has
/// always spelled it `BundleSourceType` on the wire.)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum BundleSourceType {
  Builtin,
  Remote,
}

impl From<&source::BundleSourceKind> for BundleSourceType {
  fn from(kind: &source::BundleSourceKind) -> Self {
    match kind {
      source::BundleSourceKind::Builtin => Self::Builtin,
      source::BundleSourceKind::Remote => Self::Remote,
    }
  }
}

/// Metadata for a bundle version in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BundleManifestMetadata {
  #[specta(optional)]
  pub etag: Option<String>,
  #[specta(optional)]
  pub integrity: Option<String>,
  #[specta(optional)]
  pub signature: Option<String>,
  #[specta(optional)]
  pub last_modified: Option<String>,
}

impl From<&source::BundleManifestVersionData> for BundleManifestMetadata {
  fn from(m: &source::BundleManifestVersionData) -> Self {
    Self {
      etag: m.etag.clone(),
      integrity: m.integrity.clone(),
      signature: m.signature.clone(),
      last_modified: m.last_modified.clone(),
    }
  }
}

/// Bundle version with the source (builtin/remote) that provides it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BundleSourceVersion {
  #[serde(rename = "type")]
  pub kind: BundleSourceType,
  pub version: String,
}

impl From<&source::BundleSourceVersion> for BundleSourceVersion {
  fn from(v: &source::BundleSourceVersion) -> Self {
    Self {
      kind: (&v.kind).into(),
      version: v.version.clone(),
    }
  }
}

/// A bundle entry from a source `listBundles`. Flat by design: core nests the manifest fields under
/// an `item`, but every binding's wire (and `@wvb/node`) flattens them onto the parent.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListBundleItem {
  #[serde(rename = "type")]
  pub kind: BundleSourceType,
  pub name: String,
  pub version: String,
  pub current: bool,
  pub metadata: BundleManifestMetadata,
}

impl From<&source::ListBundleItem> for ListBundleItem {
  fn from(it: &source::ListBundleItem) -> Self {
    Self {
      kind: (&it.kind).into(),
      name: it.item.name.clone(),
      version: it.item.version.clone(),
      current: it.item.current,
      metadata: (&it.item.data).into(),
    }
  }
}

pub struct WvbSource {
  pub(crate) inner: Arc<BundleSource>,
}

/// Create a `BundleSource` (`builtin_dir` read-only, `remote_dir` writable) with the default
/// options (remote bundles checked against their manifest integrity on load, under the optional
/// policy; no signature verification; header, index and entry data checksums verified on load,
/// with seed `0`).
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
/// `integrity`, `signature`, and/or the per-section `dataReadOptions`, `headerReadOptions` and
/// `indexReadOptions` (each `{ checksum?: { verify?, seed? } }`); an unparsable option returns null
/// rather than silently reading bundles unverified.
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
  // Each per-section read-option group carries a single `checksum` object, and all three sections
  // check their checksum with the same core type — so a group only overrides the section it names,
  // and only when it actually gave a `checksum`.
  match value.get("dataReadOptions") {
    None | Some(serde_json::Value::Null) => {}
    Some(x) => {
      if let Some(checksum) = parse_read_checksum_group(x)? {
        options = options.data_read(wvb::DataReadOptions::default().checksum(checksum));
      }
    }
  }
  match value.get("headerReadOptions") {
    None | Some(serde_json::Value::Null) => {}
    Some(x) => {
      if let Some(checksum) = parse_read_checksum_group(x)? {
        options = options.header_read(wvb::HeaderReadOptions::default().checksum(checksum));
      }
    }
  }
  match value.get("indexReadOptions") {
    None | Some(serde_json::Value::Null) => {}
    Some(x) => {
      if let Some(checksum) = parse_read_checksum_group(x)? {
        options = options.index_read(wvb::IndexReadOptions::default().checksum(checksum));
      }
    }
  }
  Some(options)
}

/// Read a per-section read-option group `{ checksum?: { verify?, seed? } }`. A non-object group or an
/// unknown group key returns `None` (fail closed). `Some(None)` means no `checksum` was given, and
/// `Some(Some(checksum))` carries the checksum options the caller actually set.
fn parse_read_checksum_group(
  value: &serde_json::Value,
) -> Option<Option<wvb::ChecksumReadOptions>> {
  let object = value.as_object()?;
  for key in object.keys() {
    if key != "checksum" {
      return None;
    }
  }
  match object.get("checksum") {
    None | Some(serde_json::Value::Null) => Some(None),
    Some(x) => Some(Some(parse_read_checksum(x)?)),
  }
}

/// Read a `{ verify?, seed? }` read-checksum object, applying only the keys the caller gave. A
/// non-object, an unknown sub-key, or an ill-typed value returns `None`, so the caller can fail
/// closed rather than read a section unverified.
fn parse_read_checksum(value: &serde_json::Value) -> Option<wvb::ChecksumReadOptions> {
  let object = value.as_object()?;
  for key in object.keys() {
    if key != "verify" && key != "seed" {
      return None;
    }
  }
  let mut checksum = wvb::ChecksumReadOptions::default();
  match object.get("verify") {
    None | Some(serde_json::Value::Null) => {}
    Some(x) => checksum = checksum.verify(x.as_bool()?),
  }
  match object.get("seed") {
    None | Some(serde_json::Value::Null) => {}
    Some(x) => checksum = checksum.seed(u32::try_from(x.as_u64()?).ok()?),
  }
  Some(checksum)
}

/// `integrity.checkMode`/`signature.verifyMode` string mapping — both select the same core mode.
/// Returns `None` for an unknown value, so the caller can fail closed rather than pick a default.
fn parse_verify_mode(mode: &str) -> Option<CoreBundleSourceVerifyMode> {
  match mode {
    "all" => Some(CoreBundleSourceVerifyMode::All),
    "onlyRemote" => Some(CoreBundleSourceVerifyMode::OnlyRemote),
    _ => None,
  }
}

/// # Safety
/// `handle` must be null or a pointer previously returned by `wvb_source_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_free(handle: *mut WvbSource) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}

fn manifest_metadata_json(m: &source::BundleManifestVersionData) -> serde_json::Value {
  wire_json(BundleManifestMetadata::from(m))
}

fn source_version_json(v: &source::BundleSourceVersion) -> serde_json::Value {
  wire_json(BundleSourceVersion::from(v))
}

fn list_bundle_item_json(it: &source::ListBundleItem) -> serde_json::Value {
  wire_json(ListBundleItem::from(it))
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
  match runtime().block_on(async move { source.get_version(&name).await }) {
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
  match runtime().block_on(async move { source.get_builtin_metadata(&name, &version).await }) {
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
  match runtime().block_on(async move { source.get_remote_metadata(&name, &version).await }) {
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
    serde_json::Value::Bool(source.inner.unload(&name)),
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

/// Fetch (and fully load) the current version of a bundle from the source. On success the result
/// carries a `WvbBundle` handle.
///
/// # Safety
/// `source` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_fetch_bundle(
  source: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { source.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(async move { source.fetch_bundle(&name).await }) {
    Ok(bundle) => ok_handle(Box::into_raw(Box::new(WvbBundle {
      inner: Arc::new(bundle),
    }))),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `source` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_fetch_builtin_bundle(
  source: *const WvbSource,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { source.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  match runtime().block_on(async move { source.fetch_builtin_bundle(&name, &version).await }) {
    Ok(bundle) => ok_handle(Box::into_raw(Box::new(WvbBundle {
      inner: Arc::new(bundle),
    }))),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `source` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_fetch_remote_bundle(
  source: *const WvbSource,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { source.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  match runtime().block_on(async move { source.fetch_remote_bundle(&name, &version).await }) {
    Ok(bundle) => ok_handle(Box::into_raw(Box::new(WvbBundle {
      inner: Arc::new(bundle),
    }))),
    Err(e) => core_err(e),
  }
}

/// Fetch the descriptor (header + index, no data) for the current version, keeping the parsed index
/// resident so lazy `wvb_descriptor_get_data` reads don't re-parse. Returns a `WvbDescriptor` handle.
///
/// # Safety
/// `source` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_fetch_descriptor(
  source: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { source.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(async move { source.fetch_descriptor(&name).await }) {
    Ok(descriptor) => ok_handle(Box::into_raw(Box::new(WvbDescriptor {
      inner: Arc::new(descriptor),
    }))),
    Err(e) => core_err(e),
  }
}

/// Load (and cache) the descriptor for the current version. The returned `WvbLoadedDescriptor`
/// stays pinned to its filepath + read options across active-version swaps.
///
/// # Safety
/// `source` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_load_descriptor(
  source: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { source.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(async move { source.load(&name).await }) {
    Ok(loaded) => ok_handle(Box::into_raw(Box::new(WvbLoadedDescriptor {
      inner: loaded,
    }))),
    Err(e) => core_err(e),
  }
}

/// Persist the raw bytes of a `.wvb` file to the remote dir and record it in the manifest.
/// `metadata_json` is null/empty or `{ etag?, integrity?, signature?, lastModified? }`. Storing the
/// bytes verbatim (rather than a re-serialized bundle) is what keeps the integrity string valid on
/// later loads, so this is the natural sink for a `wvb_remote_download` body.
///
/// # Safety
/// `source` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings; `data` null or
/// `data_len` readable bytes; `metadata_json` null or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_write_remote_bundle_data(
  source: *const WvbSource,
  bundle_name: *const c_char,
  version: *const c_char,
  data: *const u8,
  data_len: usize,
  metadata_json: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { source.as_ref() }).map(|h| h.inner.clone()) else {
    return null_handle_err("source");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  let bytes = unsafe { owned_bytes(data, data_len) };
  let metadata_raw = unsafe { cstr(metadata_json) };
  let metadata: source::BundleManifestVersionData = if metadata_raw.is_empty() {
    source::BundleManifestVersionData::default()
  } else {
    match serde_json::from_str(&metadata_raw) {
      Ok(metadata) => metadata,
      Err(_) => return err_result(ErrorCode::InvalidRequest, "invalid metadata".to_string()),
    }
  };
  match runtime().block_on(async move {
    source
      .write_remote_bundle_data(&name, &version, &bytes, metadata)
      .await
  }) {
    Ok(()) => ok_result(serde_json::Value::Null, Vec::new()),
    Err(e) => core_err(e),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parsed(raw: &str) -> Option<BundleSourceOptions> {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    parse_source_options(&value)
  }

  fn debug(options: &BundleSourceOptions) -> String {
    format!("{options:?}")
  }

  /// The read-checksum options carrying the given overrides.
  fn checksum(verify: bool, seed: u32) -> wvb::ChecksumReadOptions {
    wvb::ChecksumReadOptions::default()
      .verify(verify)
      .seed(seed)
  }

  /// A `BundleSourceOptions` carrying only the given data-checksum overrides.
  fn data_checksum(verify: bool, seed: u32) -> BundleSourceOptions {
    BundleSourceOptions::default()
      .data_read(wvb::DataReadOptions::default().checksum(checksum(verify, seed)))
  }

  /// A `BundleSourceOptions` carrying only the given header-checksum overrides.
  fn header_checksum(verify: bool, seed: u32) -> BundleSourceOptions {
    BundleSourceOptions::default()
      .header_read(wvb::HeaderReadOptions::default().checksum(checksum(verify, seed)))
  }

  /// A `BundleSourceOptions` carrying only the given index-checksum overrides.
  fn index_checksum(verify: bool, seed: u32) -> BundleSourceOptions {
    BundleSourceOptions::default()
      .index_read(wvb::IndexReadOptions::default().checksum(checksum(verify, seed)))
  }

  #[test]
  fn source_verifies_data_checksums_by_default() {
    assert_eq!(
      debug(&parsed("{}").unwrap()),
      debug(&BundleSourceOptions::default()),
    );

    // Overriding the seed must not turn verification back off.
    assert_eq!(
      debug(&parsed(r#"{"dataReadOptions":{"checksum":{"seed":7}}}"#).unwrap()),
      debug(&data_checksum(true, 7)),
    );

    // Nor must an unrelated option (`onlyRemote` is itself the default check mode).
    assert_eq!(
      debug(&parsed(r#"{"integrity":{"checkMode":"onlyRemote"}}"#).unwrap()),
      debug(&BundleSourceOptions::default()),
    );
  }

  #[test]
  fn source_data_checksum_can_be_turned_off() {
    assert_eq!(
      debug(&parsed(r#"{"dataReadOptions":{"checksum":{"verify":false}}}"#).unwrap()),
      debug(&data_checksum(false, 0)),
    );
  }

  #[test]
  fn source_header_and_index_read_options_round_trip() {
    assert_eq!(
      debug(&parsed(r#"{"headerReadOptions":{"checksum":{"verify":false,"seed":3}}}"#).unwrap()),
      debug(&header_checksum(false, 3)),
    );
    assert_eq!(
      debug(&parsed(r#"{"indexReadOptions":{"checksum":{"seed":5}}}"#).unwrap()),
      debug(&index_checksum(true, 5)),
    );
  }

  #[test]
  fn source_options_accept_the_nested_verification_shape() {
    assert!(parsed(r#"{"integrity":{"policy":"strict","checkMode":"all"}}"#).is_some());
    assert!(parsed(r#"{"integrity":{"policy":"off"}}"#).is_some());
    assert!(parsed(r#"{"signature":{"verifyMode":"all"}}"#).is_some());
  }

  #[test]
  fn source_options_fail_closed_on_a_bad_value() {
    assert!(parsed(r#"{"dataReadOptions":{"checksum":{"verify":"yes"}}}"#).is_none());
    assert!(parsed(r#"{"dataReadOptions":{"checksum":{"seed":-1}}}"#).is_none());
    assert!(parsed(r#"{"dataReadOptions":{"checksum":{"seed":4294967296}}}"#).is_none());
    // A present group and its checksum must each be a JSON object, and any unknown key fails closed.
    assert!(parsed(r#"{"dataReadOptions":"true"}"#).is_none());
    assert!(parsed(r#"{"dataReadOptions":{"bogus":{}}}"#).is_none());
    assert!(parsed(r#"{"dataReadOptions":{"checksum":"true"}}"#).is_none());
    assert!(parsed(r#"{"headerReadOptions":{"checksum":{"verifyy":true}}}"#).is_none());
    assert!(parsed(r#"{"indexReadOptions":{"checksum":{"seed":4294967296}}}"#).is_none());
    assert!(parsed(r#"{"integrity":"strict"}"#).is_none());
    assert!(parsed(r#"{"integrity":{"policy":"none"}}"#).is_none());
    assert!(parsed(r#"{"integrity":{"checkMode":"remote"}}"#).is_none());
    assert!(parsed(r#"{"signature":{"verifyMode":"sometimes"}}"#).is_none());
  }
}
