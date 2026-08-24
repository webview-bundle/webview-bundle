use crate::bundle::{
  Bundle, BundleDescriptor, BundleDescriptorInner, DataReadOptions, HeaderReadOptions,
  IndexReadOptions,
};
use crate::integrity::IntegrityPolicy;
use std::collections::HashMap;
use std::sync::Arc;
use wvb::source;

/// Whether a bundle was loaded from the builtin (read-only, shipped with the app)
/// or the remote (writable, downloaded at runtime) directory.
#[derive(uniffi::Enum, Clone, Debug)]
pub enum SourceKind {
  Builtin,
  Remote,
}

impl From<source::SourceKind> for SourceKind {
  fn from(value: source::SourceKind) -> Self {
    match value {
      source::SourceKind::Builtin => SourceKind::Builtin,
      source::SourceKind::Remote => SourceKind::Remote,
    }
  }
}

/// The currently active version of a bundle and where it was loaded from.
#[derive(uniffi::Record, Clone, Debug)]
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

#[derive(uniffi::Record, Clone, Debug)]
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

#[derive(uniffi::Enum, Clone, Debug)]
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

#[derive(uniffi::Record, Clone, Debug)]
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

#[derive(uniffi::Record, Clone, Debug)]
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

#[derive(uniffi::Enum, Clone, Debug)]
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

#[derive(uniffi::Record, Clone, Debug)]
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

#[derive(uniffi::Record, Clone, Debug)]
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

#[derive(uniffi::Enum, Clone, Debug)]
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

#[derive(uniffi::Record, Clone, Debug)]
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

#[derive(uniffi::Enum, Clone, Debug)]
pub enum ManifestRemoveResultKind {
  Removed,
  /// The bundle was not exists in the manifest.
  NotExists,
  /// The version was not exists in the manifest.
  VersionNotExists,
  /// The bundle is the current version so that cant be not removed.
  /// This can be force by enable `force` option.
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

#[derive(uniffi::Record, Clone, Debug)]
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

#[derive(uniffi::Record, Clone, Debug)]
pub struct ManifestRemoveData {
  pub versions: Vec<String>,
  #[uniffi(default = None)]
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

#[derive(uniffi::Record, Clone, Debug)]
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

/// A descriptor loaded (and cached) by a [`Source`].
///
/// Holds the parsed header/index together with the filepath it was loaded from, so
/// reading entry data always targets the exact bundle version that produced this
/// descriptor — even if the source's active version is swapped concurrently. Entry
/// data is read lazily from disk via [`LoadedDescriptor::get_data`], avoiding loading
/// the whole bundle into memory.
#[derive(uniffi::Object)]
pub struct LoadedDescriptor {
  pub(crate) inner: Arc<source::LoadedDescriptor>,
}

#[uniffi::export]
impl LoadedDescriptor {
  /// Returns the bundle descriptor (header + index metadata).
  ///
  /// The returned descriptor carries no reference back to the source, so it can
  /// outlive this `LoadedDescriptor`. It holds only metadata, so its `index()` is
  /// unsupported; use [`get_data`](LoadedDescriptor::get_data) for entry data.
  pub fn descriptor(&self) -> Arc<BundleDescriptor> {
    Arc::new(BundleDescriptor {
      inner: BundleDescriptorInner::Arc(self.inner.descriptor().clone()),
    })
  }
}

#[uniffi::export(async_runtime = "tokio")]
impl LoadedDescriptor {
  /// Reads the bytes for `path`, loading them lazily from disk.
  ///
  /// The read targets the bundle file this descriptor was loaded from, so the data
  /// stays consistent with [`descriptor`](LoadedDescriptor::descriptor) even if the
  /// source's active version changes meanwhile. Returns `None` if `path` does not
  /// exist in the bundle.
  pub async fn get_data(&self, path: String) -> Result<Option<Vec<u8>>, crate::Error> {
    let data = self.inner.get_data(&path).await?;
    Ok(data)
  }

  /// Reads the xxHash-32 checksum for `path`, loading it lazily from disk.
  /// Returns `None` if `path` does not exist in the bundle.
  pub async fn get_data_checksum(&self, path: String) -> Result<Option<u32>, crate::Error> {
    let checksum = self.inner.get_data_checksum(&path).await?;
    Ok(checksum)
  }
}

#[derive(uniffi::Record, Clone)]
pub struct SourceConfig {
  pub builtin_dir: String,
  pub remote_dir: String,
  #[uniffi(default = None)]
  pub builtin_manifest_filepath: Option<String>,
  #[uniffi(default = None)]
  pub remote_manifest_filepath: Option<String>,
  /// How bundles are checked against their manifest integrity metadata on load.
  #[uniffi(default = None)]
  pub integrity: Option<SourceIntegrityOptions>,
  /// How each entry's checksum is verified when its data is read.
  #[uniffi(default = None)]
  pub data_read_options: Option<DataReadOptions>,
  /// How a bundle's header checksum is verified when its descriptor is read on load.
  #[uniffi(default = None)]
  pub header_read_options: Option<HeaderReadOptions>,
  /// How a bundle's index checksum is verified when its descriptor is read on load.
  #[uniffi(default = None)]
  pub index_read_options: Option<IndexReadOptions>,
  #[uniffi(default = None)]
  pub remove_bundle_chunk_size: Option<u32>,
}

/// Which bundles a load-time verification applies to
#[derive(uniffi::Enum, Clone, Debug)]
pub enum SourceIntegrityCheckMode {
  /// Verify both builtin and remote bundles.
  ///
  /// Builtin bundles ship inside the application, so the builtin manifest must carry the
  /// metadata being verified for the check to have anything to work with.
  All,
  /// Verify downloaded (remote) bundles only. This is the default.
  OnlyRemote,
}

impl From<SourceIntegrityCheckMode> for source::SourceIntegrityCheckMode {
  fn from(v: SourceIntegrityCheckMode) -> Self {
    match v {
      SourceIntegrityCheckMode::All => source::SourceIntegrityCheckMode::All,
      SourceIntegrityCheckMode::OnlyRemote => source::SourceIntegrityCheckMode::OnlyRemote,
    }
  }
}

/// How bundles are checked against the integrity recorded for them in the manifest when
/// they are loaded from disk.
#[derive(uniffi::Record, Clone)]
pub struct SourceIntegrityOptions {
  /// How a bundle's integrity metadata is treated (default: [`IntegrityPolicy::Optional`]).
  ///
  /// [`IntegrityPolicy::Off`] disables the integrity check entirely.
  #[uniffi(default = None)]
  pub policy: Option<IntegrityPolicy>,
  #[uniffi(default = None)]
  pub check_mode: Option<SourceIntegrityCheckMode>,
}

/// Unified access point for bundles from both the builtin and remote sources.
///
/// The remote source takes precedence over the builtin source when both contain
/// a bundle with the same name.
#[derive(uniffi::Object)]
pub struct Source {
  pub(crate) inner: Arc<source::Source>,
}

fn source_builder(config: SourceConfig) -> source::SourceBuilder {
  let mut builder = source::Source::builder()
    .builtin_dir(config.builtin_dir)
    .remote_dir(config.remote_dir);
  if let Some(p) = config.builtin_manifest_filepath {
    builder = builder.builtin_manifest_filepath(p);
  }
  if let Some(p) = config.remote_manifest_filepath {
    builder = builder.remote_manifest_filepath(p);
  }
  builder
}

fn source_options(config: &mut SourceConfig) -> Result<source::SourceOptions, crate::Error> {
  let mut source_options = source::SourceOptions::default();
  if let Some(integrity) = config.integrity.take() {
    let mut integrity_options = source::SourceIntegrityOptions::default();
    if let Some(policy) = integrity.policy {
      integrity_options = integrity_options.policy(policy.into());
    }
    if let Some(check_mode) = integrity.check_mode {
      integrity_options = integrity_options.check_mode(check_mode.into());
    }
    source_options = source_options.integrity(integrity_options);
  }
  if let Some(data_read) = config.data_read_options.take() {
    source_options = source_options.data_read(data_read.into());
  }
  if let Some(header_read) = config.header_read_options.take() {
    source_options = source_options.header_read(header_read.into());
  }
  if let Some(index_read) = config.index_read_options.take() {
    source_options = source_options.index_read(index_read.into());
  }
  if let Some(size) = config.remove_bundle_chunk_size.take() {
    source_options = source_options.remove_bundle_chunk_size(size as usize);
  }
  Ok(source_options)
}

#[uniffi::export]
impl Source {
  #[uniffi::constructor]
  pub fn new(mut config: SourceConfig) -> crate::Result<Arc<Source>> {
    let options = source_options(&mut config)?;
    let builder = source_builder(config).options(options);
    Ok(Arc::new(Source {
      inner: Arc::new(builder.build()),
    }))
  }

  /// Resolves the on-disk path of the builtin bundle `bundle_name` at `version`,
  /// without checking whether the file exists.
  pub fn get_builtin_bundle_filepath(
    &self,
    bundle_name: String,
    version: String,
  ) -> Result<String, crate::Error> {
    let path = self
      .inner
      .get_builtin_bundle_filepath(&bundle_name, &version)?;
    Ok(path.to_string_lossy().to_string())
  }

  /// Resolves the on-disk path of the remote bundle `bundle_name` at `version`,
  /// without checking whether the file exists.
  pub fn get_remote_bundle_filepath(
    &self,
    bundle_name: String,
    version: String,
  ) -> Result<String, crate::Error> {
    let path = self
      .inner
      .get_remote_bundle_filepath(&bundle_name, &version)?;
    Ok(path.to_string_lossy().to_string())
  }

  /// Drops the cached descriptor for `bundle_name`, if present. Already-returned
  /// [`LoadedDescriptor`] handles keep working; the next call to [`Self::fetch_descriptor`]
  /// reloads from disk. Returns `true` if a cached descriptor was removed.
  pub fn unload(&self, bundle_name: String) -> bool {
    self.inner.unload(&bundle_name)
  }
}

#[uniffi::export(async_runtime = "tokio")]
impl Source {
  pub async fn list_bundles(&self) -> crate::Result<Vec<SourceListItem>> {
    let items = self
      .inner
      .list_bundles()
      .await?
      .into_iter()
      .map(SourceListItem::from)
      .collect();
    Ok(items)
  }

  pub async fn list_builtin_bundles(&self) -> crate::Result<Vec<SourceListItem>> {
    let items = self
      .inner
      .list_builtin_bundles()
      .await?
      .into_iter()
      .map(SourceListItem::from)
      .collect();
    Ok(items)
  }

  pub async fn list_remote_bundles(&self) -> crate::Result<Vec<SourceListItem>> {
    let items = self
      .inner
      .list_remote_bundles()
      .await?
      .into_iter()
      .map(SourceListItem::from)
      .collect();
    Ok(items)
  }

  pub async fn get_version(
    &self,
    bundle_name: String,
  ) -> crate::Result<Option<BundleSourceVersion>> {
    let version = self.inner.get_version(&bundle_name).await?;
    Ok(version.map(Into::into))
  }

  pub async fn get_remote_staged_version(
    &self,
    bundle_name: String,
  ) -> crate::Result<Option<String>> {
    let version = self.inner.get_remote_staged_version(&bundle_name).await?;
    Ok(version)
  }

  pub async fn get_remote_previous_version(
    &self,
    bundle_name: String,
  ) -> crate::Result<Option<String>> {
    let version = self.inner.get_remote_previous_version(&bundle_name).await?;
    Ok(version)
  }

  pub async fn update_remote_version(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<ManifestSetCurrentVersionResult> {
    let result = self
      .inner
      .update_remote_version(&bundle_name, &version)
      .await?;
    Ok(ManifestSetCurrentVersionResult::from(result))
  }

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

  pub async fn stage_remote_bundle(
    &self,
    bundle_name: String,
    data: ManifestStageData,
  ) -> crate::Result<ManifestStageResult> {
    let result = self
      .inner
      .stage_remote_bundle(&bundle_name, data.into())
      .await?;
    Ok(ManifestStageResult::from(result))
  }

  pub async fn stage_remote_bundles(
    &self,
    items: HashMap<String, ManifestStageData>,
  ) -> crate::Result<Vec<ManifestStageResult>> {
    let items = items
      .into_iter()
      .map(|(name, data)| (name, source::ManifestStageData::from(data)))
      .collect::<HashMap<_, _>>();
    let results = self
      .inner
      .stage_remote_bundles(items)
      .await?
      .into_iter()
      .map(ManifestStageResult::from)
      .collect::<Vec<_>>();
    Ok(results)
  }

  pub async fn resolve_filepath(&self, bundle_name: String) -> crate::Result<String> {
    let path = self.inner.resolve_filepath(&bundle_name).await?;
    Ok(path.to_string_lossy().to_string())
  }

  /// Loads the full bundle (header + index + data) for `bundle_name`.
  pub async fn fetch_bundle(&self, bundle_name: String) -> crate::Result<Arc<Bundle>> {
    let inner = self.inner.fetch_bundle(&bundle_name).await?;
    Ok(Arc::new(Bundle {
      inner: Arc::new(inner),
    }))
  }

  pub async fn fetch_builtin_bundle(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<Arc<Bundle>> {
    let inner = self
      .inner
      .fetch_builtin_bundle(&bundle_name, &version)
      .await?;
    Ok(Arc::new(Bundle {
      inner: Arc::new(inner),
    }))
  }

  pub async fn fetch_remote_bundle(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<Arc<Bundle>> {
    let inner = self
      .inner
      .fetch_remote_bundle(&bundle_name, &version)
      .await?;
    Ok(Arc::new(Bundle {
      inner: Arc::new(inner),
    }))
  }

  /// Loads only the header and index for `bundle_name`, skipping the data section.
  /// The returned descriptor does not support [`BundleDescriptor::index`]; use
  /// [`fetch`](Source::fetch_bundle) when entry data is needed.
  pub async fn fetch_descriptor(
    &self,
    bundle_name: String,
  ) -> crate::Result<Arc<BundleDescriptor>> {
    let inner = self.inner.fetch_descriptor(&bundle_name).await?;
    Ok(Arc::new(BundleDescriptor {
      inner: BundleDescriptorInner::Owned(inner),
    }))
  }

  pub async fn get_builtin_version_data(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<Option<ManifestVersionData>> {
    let version_data = self
      .inner
      .get_builtin_version_data(&bundle_name, &version)
      .await?
      .map(ManifestVersionData::from);
    Ok(version_data)
  }

  pub async fn get_remote_version_data(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<Option<ManifestVersionData>> {
    let version_data = self
      .inner
      .get_remote_version_data(&bundle_name, &version)
      .await?
      .map(ManifestVersionData::from);
    Ok(version_data)
  }

  /// Loads (and caches) the descriptor for the current version of `bundle_name`.
  /// Concurrent calls for the same bundle share a single load (single-flight) and
  /// return the cached descriptor until the active version changes or
  /// [`unload_descriptor`](Source::unload) is called.
  pub async fn load(&self, bundle_name: String) -> Result<Arc<LoadedDescriptor>, crate::Error> {
    let inner = self.inner.load(&bundle_name).await?;
    Ok(Arc::new(LoadedDescriptor { inner }))
  }

  #[uniffi::method(default(force = None))]
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
    Ok(ManifestRemoveResult::from(result))
  }

  pub async fn remove_remote_bundles(
    &self,
    items: HashMap<String, ManifestRemoveData>,
  ) -> crate::Result<Vec<ManifestRemoveResult>> {
    let items = items
      .into_iter()
      .map(|(name, data)| (name, source::ManifestRemoveData::from(data)))
      .collect::<HashMap<_, _>>();
    let results = self
      .inner
      .remove_remote_bundles(items)
      .await?
      .into_iter()
      .map(ManifestRemoveResult::from)
      .collect::<Vec<_>>();
    Ok(results)
  }

  pub async fn prune_remote_bundle(
    &self,
    bundle_name: String,
  ) -> crate::Result<ManifestPruneResult> {
    let result = self.inner.prune_remote_bundle(&bundle_name).await?;
    Ok(ManifestPruneResult::from(result))
  }

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
