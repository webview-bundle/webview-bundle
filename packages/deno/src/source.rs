#![allow(dead_code)]

use crate::bundle::{WvbBundle, WvbDescriptor, WvbLoadedDescriptor};
use crate::error::ErrorCode;
use crate::integrity::IntegrityPolicy;
use crate::options::{DataReadOptions, HeaderReadOptions, IndexReadOptions};
use crate::result::{WvbResult, core_err, err_result, null_handle_err, ok_handle, ok_result};
use crate::{cstr, runtime};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::ffi::c_char;
use std::sync::Arc;
use wvb::source;

/// The type of bundle source: builtin or remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
  Builtin,
  Remote,
}

impl From<source::SourceKind> for SourceKind {
  fn from(value: source::SourceKind) -> Self {
    match value {
      source::SourceKind::Builtin => Self::Builtin,
      source::SourceKind::Remote => Self::Remote,
    }
  }
}

/// Bundle version with the source (builtin/remote) that provides it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BundleSourceVersion {
  pub source: SourceKind,
  pub version: String,
}

impl From<source::BundleSourceVersion> for BundleSourceVersion {
  fn from(value: source::BundleSourceVersion) -> Self {
    Self {
      source: value.source.into(),
      version: value.version,
    }
  }
}

/// What the manifest records for one version of a bundle.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManifestVersionData {
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub integrity: Option<String>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub metadata: Option<HashMap<String, String>>,
}

impl From<source::ManifestVersionData> for ManifestVersionData {
  fn from(value: source::ManifestVersionData) -> Self {
    Self {
      integrity: value.integrity,
      metadata: value.metadata,
    }
  }
}

impl From<ManifestVersionData> for source::ManifestVersionData {
  fn from(value: ManifestVersionData) -> Self {
    Self {
      integrity: value.integrity,
      metadata: value.metadata,
    }
  }
}

/// Where a version stands in its bundle's lifecycle.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ManifestBundleItemStatus {
  Current,
  Previous,
  Staged,
  Orphan,
}

impl From<source::ManifestBundleItemStatus> for ManifestBundleItemStatus {
  fn from(value: source::ManifestBundleItemStatus) -> Self {
    match value {
      source::ManifestBundleItemStatus::Current => Self::Current,
      source::ManifestBundleItemStatus::Previous => Self::Previous,
      source::ManifestBundleItemStatus::Staged => Self::Staged,
      source::ManifestBundleItemStatus::Orphan => Self::Orphan,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ManifestBundleItem {
  pub name: String,
  pub version: String,
  pub status: ManifestBundleItemStatus,
  pub data: ManifestVersionData,
}

impl From<source::ManifestBundleItem> for ManifestBundleItem {
  fn from(value: source::ManifestBundleItem) -> Self {
    Self {
      name: value.name,
      version: value.version,
      status: value.status.into(),
      data: value.data.into(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceListItem {
  pub source: SourceKind,
  pub item: ManifestBundleItem,
}

impl From<source::SourceListItem> for SourceListItem {
  fn from(value: source::SourceListItem) -> Self {
    Self {
      source: value.source.into(),
      item: value.item.into(),
    }
  }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ManifestSetCurrentVersionResultKind {
  Settled,
  NotExists,
  VersionNotExists,
}

impl From<source::ManifestSetCurrentVersionResultKind> for ManifestSetCurrentVersionResultKind {
  fn from(value: source::ManifestSetCurrentVersionResultKind) -> Self {
    match value {
      source::ManifestSetCurrentVersionResultKind::Settled => Self::Settled,
      source::ManifestSetCurrentVersionResultKind::NotExists => Self::NotExists,
      source::ManifestSetCurrentVersionResultKind::VersionNotExists => Self::VersionNotExists,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSetCurrentVersionResult {
  pub name: String,
  pub version: String,
  pub kind: ManifestSetCurrentVersionResultKind,
}

impl From<source::ManifestSetCurrentVersionResult> for ManifestSetCurrentVersionResult {
  fn from(value: source::ManifestSetCurrentVersionResult) -> Self {
    Self {
      name: value.name,
      version: value.version,
      kind: value.kind.into(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManifestStageData {
  pub version: String,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<ManifestVersionData>,
}

impl From<ManifestStageData> for source::ManifestStageData {
  fn from(value: ManifestStageData) -> Self {
    Self {
      version: value.version,
      data: value.data.map(Into::into),
    }
  }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ManifestStageResultKind {
  Staged,
  InUse,
}

impl From<source::ManifestStageResultKind> for ManifestStageResultKind {
  fn from(value: source::ManifestStageResultKind) -> Self {
    match value {
      source::ManifestStageResultKind::Staged => Self::Staged,
      source::ManifestStageResultKind::InUse => Self::InUse,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ManifestStageResult {
  pub name: String,
  pub version: String,
  pub kind: ManifestStageResultKind,
}

impl From<source::ManifestStageResult> for ManifestStageResult {
  fn from(value: source::ManifestStageResult) -> Self {
    Self {
      name: value.name,
      version: value.version,
      kind: value.kind.into(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManifestRemoveData {
  pub versions: Vec<String>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub force: Option<bool>,
}

impl From<ManifestRemoveData> for source::ManifestRemoveData {
  fn from(value: ManifestRemoveData) -> Self {
    Self {
      versions: value.versions,
      force: value.force,
    }
  }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ManifestRemoveResultKind {
  Removed,
  NotExists,
  VersionNotExists,
  InUse,
}

impl From<source::ManifestRemoveResultKind> for ManifestRemoveResultKind {
  fn from(value: source::ManifestRemoveResultKind) -> Self {
    match value {
      source::ManifestRemoveResultKind::Removed => Self::Removed,
      source::ManifestRemoveResultKind::NotExists => Self::NotExists,
      source::ManifestRemoveResultKind::VersionNotExists => Self::VersionNotExists,
      source::ManifestRemoveResultKind::InUse => Self::InUse,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRemoveResult {
  pub name: String,
  pub version: String,
  pub kind: ManifestRemoveResultKind,
}

impl From<source::ManifestRemoveResult> for ManifestRemoveResult {
  fn from(value: source::ManifestRemoveResult) -> Self {
    Self {
      name: value.name,
      version: value.version,
      kind: value.kind.into(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ManifestPruneResult {
  pub name: String,
  pub pruned_versions: Vec<String>,
}

impl From<source::ManifestPruneResult> for ManifestPruneResult {
  fn from(value: source::ManifestPruneResult) -> Self {
    Self {
      name: value.name,
      pruned_versions: value.pruned_versions,
    }
  }
}

/// Which bundles are checked against the integrity recorded for them in the manifest when they are
/// loaded from disk.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SourceIntegrityCheckMode {
  /// Verify both builtin and remote bundles.
  All,
  /// Check downloaded (remote) bundles only.
  OnlyRemote,
}

impl From<SourceIntegrityCheckMode> for source::SourceIntegrityCheckMode {
  fn from(value: SourceIntegrityCheckMode) -> Self {
    match value {
      SourceIntegrityCheckMode::All => Self::All,
      SourceIntegrityCheckMode::OnlyRemote => Self::OnlyRemote,
    }
  }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceIntegrityOptions {
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub policy: Option<IntegrityPolicy>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub check_mode: Option<SourceIntegrityCheckMode>,
}

impl From<SourceIntegrityOptions> for source::SourceIntegrityOptions {
  fn from(value: SourceIntegrityOptions) -> Self {
    let mut options = Self::default();
    if let Some(policy) = value.policy {
      options = options.policy(policy.into());
    }
    if let Some(check_mode) = value.check_mode {
      options = options.check_mode(check_mode.into());
    }
    options
  }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceOptions {
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub header_read: Option<HeaderReadOptions>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub index_read: Option<IndexReadOptions>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data_read: Option<DataReadOptions>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub integrity: Option<SourceIntegrityOptions>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub remove_bundle_chunk_size: Option<u32>,
}

impl From<SourceOptions> for source::SourceOptions {
  fn from(value: SourceOptions) -> Self {
    let mut options = Self::default();
    if let Some(header_read) = value.header_read {
      options = options.header_read(header_read.into());
    }
    if let Some(index_read) = value.index_read {
      options = options.index_read(index_read.into());
    }
    if let Some(data_read) = value.data_read {
      options = options.data_read(data_read.into());
    }
    if let Some(integrity) = value.integrity {
      options = options.integrity(integrity.into());
    }
    if let Some(chunk_size) = value.remove_bundle_chunk_size {
      options = options.remove_bundle_chunk_size(chunk_size as usize);
    }
    options
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceConfig {
  pub builtin_dir: String,
  pub remote_dir: String,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub builtin_manifest_filepath: Option<String>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub remote_manifest_filepath: Option<String>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub options: Option<SourceOptions>,
}

pub struct WvbSource {
  pub(crate) inner: Arc<source::Source>,
}

/// The source a handle points at, or the null-handle error result.
macro_rules! source_of {
  ($handle:expr) => {
    match unsafe { $handle.as_ref() } {
      Some(handle) => handle.inner.clone(),
      None => return null_handle_err("source"),
    }
  };
}

/// Deserialize a JSON argument, reporting the parse failure (which names the offending key) rather
/// than falling back to a default the caller did not ask for.
fn parse_arg<T: serde::de::DeserializeOwned>(what: &str, raw: &str) -> Result<T, *mut WvbResult> {
  serde_json::from_str(raw)
    .map_err(|e| err_result(ErrorCode::InvalidRequest, format!("{what}: {e}")))
}

/// Create a `Source` from `{ builtinDir, remoteDir, builtinManifestFilepath?,
/// remoteManifestFilepath?, options? }`. An unknown or ill-typed option fails the call rather than
/// silently reading bundles with the defaults.
///
/// # Safety
/// `config_json` must be a valid NUL-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_new(config_json: *const c_char) -> *mut WvbResult {
  let raw = unsafe { cstr(config_json) };
  let config: SourceConfig = match parse_arg("invalid source config", &raw) {
    Ok(config) => config,
    Err(result) => return result,
  };
  let mut builder = source::Source::builder()
    .builtin_dir(config.builtin_dir)
    .remote_dir(config.remote_dir);
  if let Some(filepath) = config.builtin_manifest_filepath {
    builder = builder.builtin_manifest_filepath(filepath);
  }
  if let Some(filepath) = config.remote_manifest_filepath {
    builder = builder.remote_manifest_filepath(filepath);
  }
  if let Some(options) = config.options {
    builder = builder.options(options.into());
  }
  ok_handle(Box::into_raw(Box::new(WvbSource {
    inner: Arc::new(builder.build()),
  })))
}

/// # Safety
/// `handle` must be null or a pointer previously returned by `wvb_source_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_free(handle: *mut WvbSource) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}

/// A list-bundles call over one of the source's three listings.
macro_rules! list_bundles {
  ($name:ident, $method:ident) => {
    /// # Safety
    /// `handle` must be a valid `WvbSource`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn $name(handle: *const WvbSource) -> *mut WvbResult {
      let source = source_of!(handle);
      match runtime().block_on(async move { source.$method().await }) {
        Ok(items) => json_result(
          items
            .into_iter()
            .map(SourceListItem::from)
            .collect::<Vec<_>>(),
        ),
        Err(e) => core_err(e),
      }
    }
  };
}

list_bundles!(wvb_source_list_bundles, list_bundles);
list_bundles!(wvb_source_list_builtin_bundles, list_builtin_bundles);
list_bundles!(wvb_source_list_remote_bundles, list_remote_bundles);

/// Serialize a wire value as the result payload. A value that cannot be serialized would otherwise
/// reach the caller as `null`, so it is reported instead.
fn json_result<T: Serialize>(value: T) -> *mut WvbResult {
  match serde_json::to_value(value) {
    Ok(json) => ok_result(json, Vec::new()),
    Err(e) => err_result(ErrorCode::CoreSerdeJson, e.to_string()),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_get_version(
  handle: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let source = source_of!(handle);
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(async move { source.get_version(&name).await }) {
    Ok(version) => json_result(version.map(BundleSourceVersion::from)),
    Err(e) => core_err(e),
  }
}

/// A `(bundle_name) -> Option<String>` version lookup.
macro_rules! version_lookup {
  ($name:ident, $method:ident) => {
    /// # Safety
    /// `handle` must be a valid `WvbSource`; `bundle_name` a valid C string.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn $name(
      handle: *const WvbSource,
      bundle_name: *const c_char,
    ) -> *mut WvbResult {
      let source = source_of!(handle);
      let name = unsafe { cstr(bundle_name) };
      match runtime().block_on(async move { source.$method(&name).await }) {
        Ok(version) => json_result(version),
        Err(e) => core_err(e),
      }
    }
  };
}

version_lookup!(
  wvb_source_get_remote_staged_version,
  get_remote_staged_version
);
version_lookup!(
  wvb_source_get_remote_previous_version,
  get_remote_previous_version
);

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_update_remote_version(
  handle: *const WvbSource,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let source = source_of!(handle);
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  match runtime().block_on(async move { source.update_remote_version(&name, &version).await }) {
    Ok(result) => json_result(ManifestSetCurrentVersionResult::from(result)),
    Err(e) => core_err(e),
  }
}

/// `items_json` is `{ [bundleName]: version }`.
///
/// # Safety
/// `handle` must be a valid `WvbSource`; `items_json` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_update_remote_versions(
  handle: *const WvbSource,
  items_json: *const c_char,
) -> *mut WvbResult {
  let source = source_of!(handle);
  let raw = unsafe { cstr(items_json) };
  let items: HashMap<String, String> = match parse_arg("invalid version items", &raw) {
    Ok(items) => items,
    Err(result) => return result,
  };
  match runtime().block_on(async move { source.update_remote_versions(items).await }) {
    Ok(results) => json_result(
      results
        .into_iter()
        .map(ManifestSetCurrentVersionResult::from)
        .collect::<Vec<_>>(),
    ),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name`/`data_json` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_stage_remote_bundle(
  handle: *const WvbSource,
  bundle_name: *const c_char,
  data_json: *const c_char,
) -> *mut WvbResult {
  let source = source_of!(handle);
  let name = unsafe { cstr(bundle_name) };
  let raw = unsafe { cstr(data_json) };
  let data: ManifestStageData = match parse_arg("invalid stage data", &raw) {
    Ok(data) => data,
    Err(result) => return result,
  };
  match runtime().block_on(async move { source.stage_remote_bundle(&name, data.into()).await }) {
    Ok(result) => json_result(ManifestStageResult::from(result)),
    Err(e) => core_err(e),
  }
}

/// `items_json` is `{ [bundleName]: ManifestStageData }`.
///
/// # Safety
/// `handle` must be a valid `WvbSource`; `items_json` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_stage_remote_bundles(
  handle: *const WvbSource,
  items_json: *const c_char,
) -> *mut WvbResult {
  let source = source_of!(handle);
  let raw = unsafe { cstr(items_json) };
  let items: HashMap<String, ManifestStageData> = match parse_arg("invalid stage items", &raw) {
    Ok(items) => items,
    Err(result) => return result,
  };
  let items = items
    .into_iter()
    .map(|(name, data)| (name, data.into()))
    .collect::<HashMap<String, source::ManifestStageData>>();
  match runtime().block_on(async move { source.stage_remote_bundles(items).await }) {
    Ok(results) => json_result(
      results
        .into_iter()
        .map(ManifestStageResult::from)
        .collect::<Vec<_>>(),
    ),
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
  let source = source_of!(handle);
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(async move { source.resolve_filepath(&name).await }) {
    Ok(path) => json_result(path.to_string_lossy()),
    Err(e) => core_err(e),
  }
}

/// A `(bundle_name, version) -> PathBuf` filepath lookup. Synchronous: it only builds the path.
macro_rules! filepath_lookup {
  ($name:ident, $method:ident) => {
    /// # Safety
    /// `handle` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn $name(
      handle: *const WvbSource,
      bundle_name: *const c_char,
      version: *const c_char,
    ) -> *mut WvbResult {
      let source = source_of!(handle);
      let name = unsafe { cstr(bundle_name) };
      let version = unsafe { cstr(version) };
      match source.$method(&name, &version) {
        Ok(path) => json_result(path.to_string_lossy()),
        Err(e) => core_err(e),
      }
    }
  };
}

filepath_lookup!(
  wvb_source_get_builtin_bundle_filepath,
  get_builtin_bundle_filepath
);
filepath_lookup!(
  wvb_source_get_remote_bundle_filepath,
  get_remote_bundle_filepath
);

/// A `(bundle_name, version) -> Option<ManifestVersionData>` manifest lookup.
macro_rules! version_data_lookup {
  ($name:ident, $method:ident) => {
    /// # Safety
    /// `handle` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn $name(
      handle: *const WvbSource,
      bundle_name: *const c_char,
      version: *const c_char,
    ) -> *mut WvbResult {
      let source = source_of!(handle);
      let name = unsafe { cstr(bundle_name) };
      let version = unsafe { cstr(version) };
      match runtime().block_on(async move { source.$method(&name, &version).await }) {
        Ok(data) => json_result(data.map(ManifestVersionData::from)),
        Err(e) => core_err(e),
      }
    }
  };
}

version_data_lookup!(
  wvb_source_get_builtin_version_data,
  get_builtin_version_data
);
version_data_lookup!(wvb_source_get_remote_version_data, get_remote_version_data);

/// Fetch (and fully load) the current version of a bundle from the source. On success the result
/// carries a `WvbBundle` handle.
///
/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_fetch_bundle(
  handle: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let source = source_of!(handle);
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(async move { source.fetch_bundle(&name).await }) {
    Ok(bundle) => ok_handle(Box::into_raw(Box::new(WvbBundle {
      inner: Arc::new(bundle),
    }))),
    Err(e) => core_err(e),
  }
}

/// A `(bundle_name, version) -> Bundle` fetch of one source's bundles.
macro_rules! fetch_bundle_at {
  ($name:ident, $method:ident) => {
    /// # Safety
    /// `handle` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn $name(
      handle: *const WvbSource,
      bundle_name: *const c_char,
      version: *const c_char,
    ) -> *mut WvbResult {
      let source = source_of!(handle);
      let name = unsafe { cstr(bundle_name) };
      let version = unsafe { cstr(version) };
      match runtime().block_on(async move { source.$method(&name, &version).await }) {
        Ok(bundle) => ok_handle(Box::into_raw(Box::new(WvbBundle {
          inner: Arc::new(bundle),
        }))),
        Err(e) => core_err(e),
      }
    }
  };
}

fetch_bundle_at!(wvb_source_fetch_builtin_bundle, fetch_builtin_bundle);
fetch_bundle_at!(wvb_source_fetch_remote_bundle, fetch_remote_bundle);

/// Fetch the descriptor (header + index, no data) for the current version, keeping the parsed index
/// resident so lazy `wvb_descriptor_get_data` reads don't re-parse. Returns a `WvbDescriptor` handle.
///
/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_fetch_descriptor(
  handle: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let source = source_of!(handle);
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
/// `handle` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_load(
  handle: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let source = source_of!(handle);
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(async move { source.load(&name).await }) {
    Ok(loaded) => ok_handle(Box::into_raw(Box::new(WvbLoadedDescriptor {
      inner: loaded,
    }))),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_unload(
  handle: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let source = source_of!(handle);
  let name = unsafe { cstr(bundle_name) };
  json_result(source.unload(&name))
}

/// `force` is `1`/`0` to set it, and any other value (e.g. `-1`) to leave it unset.
///
/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name`/`version` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_remove_remote_bundle(
  handle: *const WvbSource,
  bundle_name: *const c_char,
  version: *const c_char,
  force: i8,
) -> *mut WvbResult {
  let source = source_of!(handle);
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  let force = match force {
    0 => Some(false),
    1 => Some(true),
    _ => None,
  };
  match runtime().block_on(async move { source.remove_remote_bundle(&name, &version, force).await })
  {
    Ok(result) => json_result(ManifestRemoveResult::from(result)),
    Err(e) => core_err(e),
  }
}

/// `items_json` is `{ [bundleName]: ManifestRemoveData }`.
///
/// # Safety
/// `handle` must be a valid `WvbSource`; `items_json` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_remove_remote_bundles(
  handle: *const WvbSource,
  items_json: *const c_char,
) -> *mut WvbResult {
  let source = source_of!(handle);
  let raw = unsafe { cstr(items_json) };
  let items: HashMap<String, ManifestRemoveData> = match parse_arg("invalid remove items", &raw) {
    Ok(items) => items,
    Err(result) => return result,
  };
  let items = items
    .into_iter()
    .map(|(name, data)| (name, data.into()))
    .collect::<HashMap<String, source::ManifestRemoveData>>();
  match runtime().block_on(async move { source.remove_remote_bundles(items).await }) {
    Ok(results) => json_result(
      results
        .into_iter()
        .map(ManifestRemoveResult::from)
        .collect::<Vec<_>>(),
    ),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbSource`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_prune_remote_bundle(
  handle: *const WvbSource,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let source = source_of!(handle);
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(async move { source.prune_remote_bundle(&name).await }) {
    Ok(result) => json_result(ManifestPruneResult::from(result)),
    Err(e) => core_err(e),
  }
}

/// `names_json` is an array of bundle names.
///
/// # Safety
/// `handle` must be a valid `WvbSource`; `names_json` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_source_prune_remote_bundles(
  handle: *const WvbSource,
  names_json: *const c_char,
) -> *mut WvbResult {
  let source = source_of!(handle);
  let raw = unsafe { cstr(names_json) };
  let names: Vec<String> = match parse_arg("invalid bundle names", &raw) {
    Ok(names) => names,
    Err(result) => return result,
  };
  match runtime().block_on(async move { source.prune_remote_bundles(&names).await }) {
    Ok(results) => json_result(
      results
        .into_iter()
        .map(ManifestPruneResult::from)
        .collect::<Vec<_>>(),
    ),
    Err(e) => core_err(e),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parsed(raw: &str) -> Result<source::SourceOptions, String> {
    serde_json::from_str::<SourceOptions>(raw)
      .map(Into::into)
      .map_err(|e| e.to_string())
  }

  fn debug(options: &source::SourceOptions) -> String {
    format!("{options:?}")
  }

  fn checksum(verify: bool, seed: u32) -> wvb::ChecksumReadOptions {
    wvb::ChecksumReadOptions::default()
      .verify(verify)
      .seed(seed)
  }

  #[test]
  fn source_verifies_data_checksums_by_default() {
    assert_eq!(
      debug(&parsed("{}").unwrap()),
      debug(&source::SourceOptions::default()),
    );

    // Overriding the seed must not turn verification back off.
    assert_eq!(
      debug(&parsed(r#"{"dataRead":{"checksum":{"seed":7}}}"#).unwrap()),
      debug(
        &source::SourceOptions::default()
          .data_read(wvb::DataReadOptions::default().checksum(checksum(true, 7)))
      ),
    );

    // Nor must an unrelated option (`only_remote` is itself the default check mode).
    assert_eq!(
      debug(&parsed(r#"{"integrity":{"checkMode":"only_remote"}}"#).unwrap()),
      debug(&source::SourceOptions::default()),
    );
  }

  #[test]
  fn source_header_and_index_read_options_round_trip() {
    assert_eq!(
      debug(&parsed(r#"{"headerRead":{"checksum":{"verify":false,"seed":3}}}"#).unwrap()),
      debug(
        &source::SourceOptions::default()
          .header_read(wvb::HeaderReadOptions::default().checksum(checksum(false, 3)))
      ),
    );
    assert_eq!(
      debug(&parsed(r#"{"indexRead":{"checksum":{"seed":5}}}"#).unwrap()),
      debug(
        &source::SourceOptions::default()
          .index_read(wvb::IndexReadOptions::default().checksum(checksum(true, 5)))
      ),
    );
  }

  #[test]
  fn source_options_fail_closed_on_a_bad_value() {
    // A misspelled key is named in the error rather than dropped in silence: dropping
    // `dataRead.checksum.verify` would leave verification in a state the caller did not ask for.
    let error = parsed(r#"{"dataRead":{"checksum":{"verifyy":true}}}"#).unwrap_err();
    assert!(error.contains("verifyy"), "{error}");
    assert!(parsed(r#"{"dataRead":{"checksum":{"verify":"yes"}}}"#).is_err());
    assert!(parsed(r#"{"dataRead":{"checksum":{"seed":-1}}}"#).is_err());
    assert!(parsed(r#"{"dataRead":{"checksum":{"seed":4294967296}}}"#).is_err());
    assert!(parsed(r#"{"dataRead":"true"}"#).is_err());
    assert!(parsed(r#"{"integrity":"strict"}"#).is_err());
    // 'none' was the old spelling of 'off'; it must not silently pick a default.
    assert!(parsed(r#"{"integrity":{"policy":"none"}}"#).is_err());
    assert!(parsed(r#"{"integrity":{"checkMode":"remote"}}"#).is_err());
    assert!(parsed(r#"{"integrity":{"checkmode":"all"}}"#).is_err());
  }
}
