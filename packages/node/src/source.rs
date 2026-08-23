use crate::bundle::Bundle;
use crate::bundle::BundleDescriptor;
use crate::bundle::BundleDescriptorInner;
use crate::bundle::{DataReadOptions, HeaderReadOptions, IndexReadOptions};
use crate::integrity::IntegrityPolicy;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::collections::HashMap;
use std::sync::Arc;
use wvb::source;

#[napi(string_enum = "snake_case")]
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

#[napi(object)]
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

#[napi]
pub enum ManifestVersion {
  V1 = 1,
}

impl From<source::ManifestVersion> for ManifestVersion {
  fn from(value: source::ManifestVersion) -> Self {
    match value {
      source::ManifestVersion::V1 => Self::V1,
    }
  }
}

#[napi(object)]
pub struct ManifestVersionData {
  pub integrity: Option<String>,
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

#[napi(object)]
pub struct ManifestBundleSet {
  pub versions: HashMap<String, ManifestVersionData>,
  pub current_version: Option<String>,
  pub previous_version: Option<String>,
  pub staged_version: Option<String>,
}

impl From<source::ManifestBundleSet> for ManifestBundleSet {
  fn from(value: source::ManifestBundleSet) -> Self {
    Self {
      versions: value
        .versions
        .into_iter()
        .map(|(version, data)| (version, data.into()))
        .collect(),
      current_version: value.current_version,
      previous_version: value.previous_version,
      staged_version: value.staged_version,
    }
  }
}

#[napi(object)]
pub struct ManifestData {
  pub manifest_version: ManifestVersion,
  pub bundles: HashMap<String, ManifestBundleSet>,
}

impl From<source::ManifestData> for ManifestData {
  fn from(value: source::ManifestData) -> Self {
    Self {
      manifest_version: value.manifest_version.into(),
      bundles: value
        .bundles
        .into_iter()
        .map(|(name, set)| (name, set.into()))
        .collect(),
    }
  }
}

#[napi(string_enum = "snake_case")]
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

#[napi(object)]
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

#[napi(object)]
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

#[napi(string_enum = "snake_case")]
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

#[napi(object)]
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

#[napi(object)]
pub struct ManifestStageData {
  pub version: String,
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

#[napi(string_enum = "snake_case")]
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

#[napi(object)]
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

#[napi(object)]
pub struct ManifestRemoveData {
  pub versions: Vec<String>,
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

#[napi(string_enum = "snake_case")]
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

#[napi(object)]
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

#[napi(object)]
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

#[napi(string_enum = "snake_case")]
pub enum SourceIntegrityCheckMode {
  All,
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

#[napi(object)]
pub struct SourceIntegrityOptions {
  pub policy: Option<IntegrityPolicy>,
  pub check_mode: Option<SourceIntegrityCheckMode>,
}

impl From<SourceIntegrityOptions> for source::SourceIntegrityOptions {
  fn from(value: SourceIntegrityOptions) -> Self {
    let mut options = source::SourceIntegrityOptions::default();
    if let Some(policy) = value.policy {
      options = options.policy(policy.into());
    }
    if let Some(check_mode) = value.check_mode {
      options = options.check_mode(check_mode.into());
    }
    options
  }
}

#[napi(object)]
pub struct SourceOptions {
  pub header_read: Option<HeaderReadOptions>,
  pub index_read: Option<IndexReadOptions>,
  pub data_read: Option<DataReadOptions>,
  pub integrity: Option<SourceIntegrityOptions>,
  pub remove_bundle_chunk_size: Option<u32>,
}

impl From<SourceOptions> for source::SourceOptions {
  fn from(value: SourceOptions) -> Self {
    let mut options = source::SourceOptions::default();
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

#[napi(object)]
pub struct SourceConfig {
  pub builtin_dir: String,
  pub remote_dir: String,
  pub builtin_manifest_filepath: Option<String>,
  pub remote_manifest_filepath: Option<String>,
  pub options: Option<SourceOptions>,
}

#[napi]
pub struct LoadedDescriptor {
  pub(crate) inner: Arc<source::LoadedDescriptor>,
}

#[napi]
impl LoadedDescriptor {
  #[napi]
  pub fn descriptor(&self) -> BundleDescriptor {
    BundleDescriptor {
      inner: BundleDescriptorInner::Arc(self.inner.descriptor().clone()),
    }
  }

  #[napi]
  pub async fn get_data(&self, path: String) -> crate::Result<Option<Buffer>> {
    let data = self.inner.get_data(&path).await?;
    Ok(data.map(|x| x.into()))
  }

  #[napi]
  pub async fn get_data_checksum(&self, path: String) -> crate::Result<Option<u32>> {
    let checksum = self.inner.get_data_checksum(&path).await?;
    Ok(checksum)
  }
}

#[napi]
pub struct Source {
  pub(crate) inner: Arc<source::Source>,
}

#[napi]
impl Source {
  #[napi(constructor)]
  pub fn new(config: SourceConfig) -> Source {
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
    Source {
      inner: Arc::new(builder.build()),
    }
  }

  #[napi]
  pub async fn list_bundles(&self) -> crate::Result<Vec<SourceListItem>> {
    let items = self
      .inner
      .list_bundles()
      .await?
      .into_iter()
      .map(SourceListItem::from)
      .collect::<Vec<_>>();
    Ok(items)
  }

  #[napi]
  pub async fn list_builtin_bundles(&self) -> crate::Result<Vec<SourceListItem>> {
    let items = self
      .inner
      .list_builtin_bundles()
      .await?
      .into_iter()
      .map(SourceListItem::from)
      .collect::<Vec<_>>();
    Ok(items)
  }

  #[napi]
  pub async fn list_remote_bundles(&self) -> crate::Result<Vec<SourceListItem>> {
    let items = self
      .inner
      .list_remote_bundles()
      .await?
      .into_iter()
      .map(SourceListItem::from)
      .collect::<Vec<_>>();
    Ok(items)
  }

  #[napi]
  pub async fn get_version(
    &self,
    bundle_name: String,
  ) -> crate::Result<Option<BundleSourceVersion>> {
    let version = self.inner.get_version(&bundle_name).await?;
    Ok(version.map(Into::into))
  }

  #[napi]
  pub async fn get_remote_staged_version(
    &self,
    bundle_name: String,
  ) -> crate::Result<Option<String>> {
    let version = self.inner.get_remote_staged_version(&bundle_name).await?;
    Ok(version)
  }

  #[napi]
  pub async fn get_remote_previous_version(
    &self,
    bundle_name: String,
  ) -> crate::Result<Option<String>> {
    let version = self.inner.get_remote_previous_version(&bundle_name).await?;
    Ok(version)
  }

  #[napi]
  pub async fn update_remote_version(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<ManifestSetCurrentVersionResult> {
    let result = self
      .inner
      .update_remote_version(&bundle_name, &version)
      .await?;
    Ok(result.into())
  }

  #[napi]
  pub async fn update_remote_versions(
    &self,
    items: HashMap<String, String>,
  ) -> crate::Result<Vec<ManifestSetCurrentVersionResult>> {
    let results = self
      .inner
      .update_remote_versions(items)
      .await?
      .into_iter()
      .map(ManifestSetCurrentVersionResult::from)
      .collect::<Vec<_>>();
    Ok(results)
  }

  #[napi]
  pub async fn stage_remote_bundle(
    &self,
    bundle_name: String,
    data: ManifestStageData,
  ) -> crate::Result<ManifestStageResult> {
    let result = self
      .inner
      .stage_remote_bundle(&bundle_name, data.into())
      .await?;
    Ok(result.into())
  }

  #[napi]
  pub async fn stage_remote_bundles(
    &self,
    items: HashMap<String, ManifestStageData>,
  ) -> crate::Result<Vec<ManifestStageResult>> {
    let items = items
      .into_iter()
      .map(|(name, data)| (name, data.into()))
      .collect::<HashMap<String, source::ManifestStageData>>();
    let results = self
      .inner
      .stage_remote_bundles(items)
      .await?
      .into_iter()
      .map(ManifestStageResult::from)
      .collect::<Vec<_>>();
    Ok(results)
  }

  #[napi]
  pub async fn resolve_filepath(&self, bundle_name: String) -> crate::Result<String> {
    let filepath = self.inner.resolve_filepath(&bundle_name).await?;
    Ok(filepath.to_string_lossy().to_string())
  }

  #[napi]
  pub fn get_builtin_bundle_filepath(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<String> {
    let filepath = self
      .inner
      .get_builtin_bundle_filepath(&bundle_name, &version)?;
    Ok(filepath.to_string_lossy().to_string())
  }

  #[napi]
  pub fn get_remote_bundle_filepath(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<String> {
    let filepath = self
      .inner
      .get_remote_bundle_filepath(&bundle_name, &version)?;
    Ok(filepath.to_string_lossy().to_string())
  }

  #[napi]
  pub async fn fetch_bundle(&self, bundle_name: String) -> crate::Result<Bundle> {
    let inner = self.inner.fetch_bundle(&bundle_name).await?;
    Ok(Bundle { inner })
  }

  #[napi]
  pub async fn fetch_builtin_bundle(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<Bundle> {
    let inner = self
      .inner
      .fetch_builtin_bundle(&bundle_name, &version)
      .await?;
    Ok(Bundle { inner })
  }

  #[napi]
  pub async fn fetch_remote_bundle(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<Bundle> {
    let inner = self
      .inner
      .fetch_remote_bundle(&bundle_name, &version)
      .await?;
    Ok(Bundle { inner })
  }

  #[napi]
  pub async fn fetch_descriptor(&self, bundle_name: String) -> crate::Result<BundleDescriptor> {
    let inner = self.inner.fetch_descriptor(&bundle_name).await?;
    Ok(BundleDescriptor {
      inner: BundleDescriptorInner::Owned(inner),
    })
  }

  #[napi]
  pub async fn get_builtin_version_data(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<Option<ManifestVersionData>> {
    let data = self
      .inner
      .get_builtin_version_data(&bundle_name, &version)
      .await?
      .map(ManifestVersionData::from);
    Ok(data)
  }

  #[napi]
  pub async fn get_remote_version_data(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<Option<ManifestVersionData>> {
    let data = self
      .inner
      .get_remote_version_data(&bundle_name, &version)
      .await?
      .map(ManifestVersionData::from);
    Ok(data)
  }

  #[napi]
  pub async fn load(&self, bundle_name: String) -> crate::Result<LoadedDescriptor> {
    let inner = self.inner.load(&bundle_name).await?;
    Ok(LoadedDescriptor { inner })
  }

  #[napi]
  pub fn unload(&self, bundle_name: String) -> bool {
    self.inner.unload(&bundle_name)
  }

  #[napi]
  pub async fn remove_remote_bundle(
    &self,
    bundle_name: String,
    version: String,
    force: Option<bool>,
  ) -> crate::Result<ManifestRemoveResult> {
    let result = self
      .inner
      .remove_remote_bundle(&bundle_name, &version, force)
      .await?;
    Ok(result.into())
  }

  #[napi]
  pub async fn remove_remote_bundles(
    &self,
    items: HashMap<String, ManifestRemoveData>,
  ) -> crate::Result<Vec<ManifestRemoveResult>> {
    let items = items
      .into_iter()
      .map(|(name, data)| (name, data.into()))
      .collect::<HashMap<String, source::ManifestRemoveData>>();
    let results = self
      .inner
      .remove_remote_bundles(items)
      .await?
      .into_iter()
      .map(ManifestRemoveResult::from)
      .collect::<Vec<_>>();
    Ok(results)
  }

  #[napi]
  pub async fn prune_remote_bundle(
    &self,
    bundle_name: String,
  ) -> crate::Result<ManifestPruneResult> {
    let result = self.inner.prune_remote_bundle(&bundle_name).await?;
    Ok(result.into())
  }

  #[napi]
  pub async fn prune_remote_bundles(
    &self,
    bundle_names: Vec<String>,
  ) -> crate::Result<Vec<ManifestPruneResult>> {
    let results = self
      .inner
      .prune_remote_bundles(&bundle_names)
      .await?
      .into_iter()
      .map(ManifestPruneResult::from)
      .collect::<Vec<_>>();
    Ok(results)
  }
}
