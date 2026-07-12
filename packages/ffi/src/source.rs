use crate::bundle::{Bundle, BundleDescriptor, BundleDescriptorInner};
use crate::integrity::IntegrityPolicy;
use crate::signature::SignatureVerifierOptions;
use std::sync::Arc;
use wvb::signature;
use wvb::source;

/// Whether a bundle was loaded from the builtin (read-only, shipped with the app)
/// or the remote (writable, downloaded at runtime) directory.
#[derive(uniffi::Enum, Clone, Debug)]
pub enum BundleSourceKind {
  Builtin,
  Remote,
}

impl From<source::BundleSourceKind> for BundleSourceKind {
  fn from(value: source::BundleSourceKind) -> Self {
    match value {
      source::BundleSourceKind::Builtin => BundleSourceKind::Builtin,
      source::BundleSourceKind::Remote => BundleSourceKind::Remote,
    }
  }
}

/// The currently active version of a bundle and where it was loaded from.
#[derive(uniffi::Record, Clone, Debug)]
pub struct BundleSourceVersion {
  pub kind: BundleSourceKind,
  pub version: String,
}

impl From<source::BundleSourceVersion> for BundleSourceVersion {
  fn from(value: source::BundleSourceVersion) -> Self {
    Self {
      kind: value.kind.into(),
      version: value.version,
    }
  }
}

/// HTTP cache-control fields stored alongside each bundle entry in the manifest.
/// Used to avoid re-downloading bundles that haven't changed.
#[derive(uniffi::Record, Clone, Debug)]
pub struct BundleManifestMetadata {
  pub etag: Option<String>,
  pub integrity: Option<String>,
  pub signature: Option<String>,
  pub last_modified: Option<String>,
}

impl From<source::BundleManifestMetadata> for BundleManifestMetadata {
  fn from(value: source::BundleManifestMetadata) -> Self {
    Self {
      etag: value.etag,
      integrity: value.integrity,
      signature: value.signature,
      last_modified: value.last_modified,
    }
  }
}

impl From<BundleManifestMetadata> for source::BundleManifestMetadata {
  fn from(value: BundleManifestMetadata) -> Self {
    Self {
      etag: value.etag,
      integrity: value.integrity,
      signature: value.signature,
      last_modified: value.last_modified,
    }
  }
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct ListBundleItem {
  pub kind: BundleSourceKind,
  pub name: String,
  pub version: String,
  pub current: bool,
  pub metadata: BundleManifestMetadata,
}

impl From<source::ListBundleItem> for ListBundleItem {
  fn from(value: source::ListBundleItem) -> Self {
    Self {
      kind: value.kind.into(),
      name: value.item.name,
      version: value.item.version,
      current: value.item.current,
      metadata: value.item.metadata.into(),
    }
  }
}

/// A descriptor loaded (and cached) by a [`BundleSource`].
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

/// Directory paths used by [`BundleSource`] to locate bundles on disk.
///
/// `builtin_dir` is read-only (e.g. the app bundle on iOS/Android).
/// `remote_dir` must be writable so downloaded bundles can be persisted.
/// Both manifest paths default to `<dir>/manifest.json` when `None`.
#[derive(uniffi::Record, Clone, Debug)]
pub struct BundleSourceConfig {
  pub builtin_dir: String,
  pub remote_dir: String,
  pub builtin_manifest_filepath: Option<String>,
  pub remote_manifest_filepath: Option<String>,
}

/// Which bundles are verified against their manifest metadata when loaded from disk.
///
/// A bundle is verified once per version, when it is first read; the result is cached with
/// the descriptor, so serving a bundle does not re-hash it on every request.
#[derive(uniffi::Enum, Clone, Debug)]
pub enum VerifyOnLoad {
  /// Never verify on load. Bundles are still verified when downloaded and installed.
  None,
  /// Verify downloaded (remote) bundles only.
  Remote,
  /// Verify both builtin and remote bundles.
  ///
  /// Builtin bundles are hashed too, so the builtin manifest should carry integrity metadata
  /// for every bundle. Whether a *missing* integrity string is an error is decided by
  /// [`IntegrityPolicy`] rather than by this variant: under the default
  /// [`IntegrityPolicy::Optional`] a builtin bundle with no integrity metadata still loads.
  /// Pair this with [`IntegrityPolicy::Strict`] — or with a signature verifier, which forces
  /// the integrity check — to require the metadata.
  All,
}

impl From<VerifyOnLoad> for source::VerifyOnLoad {
  fn from(v: VerifyOnLoad) -> Self {
    match v {
      VerifyOnLoad::None => source::VerifyOnLoad::None,
      VerifyOnLoad::Remote => source::VerifyOnLoad::Remote,
      VerifyOnLoad::All => source::VerifyOnLoad::All,
    }
  }
}

/// Optional verification settings for a [`BundleSource`].
///
/// Two independent layers: load-time integrity/signature verification of the whole bundle
/// file ([`VerifyOnLoad`], paid once per version), and the per-read entry checksum applied
/// by [`LoadedDescriptor::get_data`].
#[derive(uniffi::Record, Clone, Debug)]
pub struct BundleSourceOptions {
  /// Default: [`VerifyOnLoad::None`].
  pub verify_on_load: Option<VerifyOnLoad>,
  pub integrity_policy: Option<IntegrityPolicy>,
  /// Verifies that a bundle's integrity string was signed by the matching key. Configuring
  /// a verifier also makes the integrity check mandatory, regardless of `integrity_policy`.
  ///
  /// **A verifier alone verifies nothing.** Load-time verification only runs on the bundles
  /// `verify_on_load` selects, which by default is none of them; set it to
  /// [`VerifyOnLoad::Remote`] (or [`VerifyOnLoad::All`]) as well.
  pub signature_verifier: Option<SignatureVerifierOptions>,
  /// Verify each entry's xxHash-32 checksum when its data is read (default: `true`).
  pub verify_data_checksum: Option<bool>,
  /// The seed the bundle's data checksums were built with (default: `0`).
  pub data_checksum_seed: Option<u32>,
}

/// Unified access point for bundles from both the builtin and remote sources.
///
/// The remote source takes precedence over the builtin source when both contain
/// a bundle with the same name.
#[derive(uniffi::Object)]
pub struct BundleSource {
  pub(crate) inner: Arc<source::BundleSource>,
}

fn source_builder(config: BundleSourceConfig) -> source::BundleSourceBuilder {
  let mut builder = source::BundleSource::builder()
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

fn source_options(
  options: BundleSourceOptions,
) -> Result<source::BundleSourceOptions, crate::Error> {
  let mut source_options = source::BundleSourceOptions::new();
  if let Some(verify_on_load) = options.verify_on_load {
    source_options = source_options.verify_on_load(verify_on_load.into());
  }
  if let Some(policy) = options.integrity_policy {
    source_options = source_options.integrity_policy(policy.into());
  }
  if let Some(verifier_opts) = options.signature_verifier {
    let verifier = signature::SignatureVerifier::try_from(verifier_opts)?;
    source_options = source_options.signature_verifier(verifier);
  }
  if let Some(verify) = options.verify_data_checksum {
    source_options = source_options.verify_data_checksum(verify);
  }
  if let Some(seed) = options.data_checksum_seed {
    source_options = source_options.data_checksum_seed(seed);
  }
  Ok(source_options)
}

#[uniffi::export]
impl BundleSource {
  #[uniffi::constructor]
  pub fn new(config: BundleSourceConfig) -> Arc<BundleSource> {
    Arc::new(BundleSource {
      inner: Arc::new(source_builder(config).build()),
    })
  }

  /// Same as [`BundleSource::new`], with verification options applied to every bundle
  /// this source loads or reads.
  #[uniffi::constructor(name = "with_options")]
  pub fn with_options(
    config: BundleSourceConfig,
    options: BundleSourceOptions,
  ) -> Result<Arc<BundleSource>, crate::Error> {
    let builder = source_builder(config).options(source_options(options)?);
    Ok(Arc::new(BundleSource {
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
  /// [`LoadedDescriptor`] handles keep working; the next [`load_descriptor`]
  /// reloads from disk. Returns `true` if a cached descriptor was removed.
  pub fn unload_descriptor(&self, bundle_name: String) -> bool {
    self.inner.unload_descriptor(&bundle_name)
  }
}

#[uniffi::export(async_runtime = "tokio")]
impl BundleSource {
  pub async fn list_bundles(&self) -> Result<Vec<ListBundleItem>, crate::Error> {
    let items = self
      .inner
      .list_bundles()
      .await?
      .into_iter()
      .map(ListBundleItem::from)
      .collect();
    Ok(items)
  }

  pub async fn load_version(
    &self,
    bundle_name: String,
  ) -> Result<Option<BundleSourceVersion>, crate::Error> {
    let version = self.inner.load_version(&bundle_name).await?;
    Ok(version.map(Into::into))
  }

  pub async fn update_version(
    &self,
    bundle_name: String,
    version: String,
  ) -> Result<(), crate::Error> {
    self
      .inner
      .update_remote_version(&bundle_name, &version)
      .await?;
    Ok(())
  }

  pub async fn resolve_filepath(&self, bundle_name: String) -> Result<String, crate::Error> {
    let path = self.inner.resolve_filepath(&bundle_name).await?;
    Ok(path.to_string_lossy().to_string())
  }

  /// Loads the full bundle (header + index + data) for `bundle_name`.
  pub async fn fetch_bundle(&self, bundle_name: String) -> Result<Arc<Bundle>, crate::Error> {
    let inner = self.inner.fetch_bundle(&bundle_name).await?;
    Ok(Arc::new(Bundle {
      inner: Arc::new(inner),
    }))
  }

  /// Loads only the header and index for `bundle_name`, skipping the data section.
  /// The returned descriptor does not support [`BundleDescriptor::index`]; use
  /// [`fetch`](BundleSource::fetch_bundle) when entry data is needed.
  pub async fn fetch_descriptor(
    &self,
    bundle_name: String,
  ) -> Result<Arc<BundleDescriptor>, crate::Error> {
    let inner = self.inner.fetch_descriptor(&bundle_name).await?;
    Ok(Arc::new(BundleDescriptor {
      inner: BundleDescriptorInner::Owned(inner),
    }))
  }

  /// Loads the full builtin bundle for `bundle_name` at `version`, bypassing the
  /// remote source and version resolution.
  pub async fn fetch_builtin_bundle(
    &self,
    bundle_name: String,
    version: String,
  ) -> Result<Arc<Bundle>, crate::Error> {
    let inner = self
      .inner
      .fetch_builtin_bundle(&bundle_name, &version)
      .await?;
    Ok(Arc::new(Bundle {
      inner: Arc::new(inner),
    }))
  }

  /// Loads the full remote bundle for `bundle_name` at `version`, bypassing the
  /// builtin source and version resolution.
  pub async fn fetch_remote_bundle(
    &self,
    bundle_name: String,
    version: String,
  ) -> Result<Arc<Bundle>, crate::Error> {
    let inner = self
      .inner
      .fetch_remote_bundle(&bundle_name, &version)
      .await?;
    Ok(Arc::new(Bundle {
      inner: Arc::new(inner),
    }))
  }

  /// Loads (and caches) the descriptor for the current version of `bundle_name`.
  /// Concurrent calls for the same bundle share a single load (single-flight) and
  /// return the cached descriptor until the active version changes or
  /// [`unload_descriptor`](BundleSource::unload_descriptor) is called.
  pub async fn load_descriptor(
    &self,
    bundle_name: String,
  ) -> Result<Arc<LoadedDescriptor>, crate::Error> {
    let inner = self.inner.load_descriptor(&bundle_name).await?;
    Ok(Arc::new(LoadedDescriptor { inner }))
  }

  pub async fn write_remote_bundle(
    &self,
    bundle_name: String,
    version: String,
    bundle: Arc<Bundle>,
    metadata: BundleManifestMetadata,
  ) -> Result<(), crate::Error> {
    self
      .inner
      .write_remote_bundle(&bundle_name, &version, &bundle.inner, metadata.into())
      .await?;
    Ok(())
  }

  /// Loads the manifest metadata for the builtin bundle `bundle_name` at `version`,
  /// or `None` if no such entry exists.
  pub async fn load_builtin_metadata(
    &self,
    bundle_name: String,
    version: String,
  ) -> Result<Option<BundleManifestMetadata>, crate::Error> {
    let metadata = self
      .inner
      .load_builtin_metadata(&bundle_name, &version)
      .await?
      .map(BundleManifestMetadata::from);
    Ok(metadata)
  }

  /// Loads the manifest metadata for the remote bundle `bundle_name` at `version`,
  /// or `None` if no such entry exists.
  pub async fn load_remote_metadata(
    &self,
    bundle_name: String,
    version: String,
  ) -> Result<Option<BundleManifestMetadata>, crate::Error> {
    let metadata = self
      .inner
      .load_remote_metadata(&bundle_name, &version)
      .await?
      .map(BundleManifestMetadata::from);
    Ok(metadata)
  }

  /// Removes a single staged remote bundle version: drops its manifest entry and
  /// deletes its file from disk. Returns `true` if the entry existed.
  pub async fn remove_remote_bundle(
    &self,
    bundle_name: String,
    version: String,
  ) -> Result<bool, crate::Error> {
    let removed = self
      .inner
      .remove_remote_bundle(&bundle_name, &version)
      .await?;
    Ok(removed)
  }

  /// Returns the remote versions that pruning retains (the current and previous).
  pub async fn remote_retained_versions(
    &self,
    bundle_name: String,
  ) -> Result<Vec<String>, crate::Error> {
    let versions = self.inner.remote_retained_versions(&bundle_name).await?;
    Ok(versions)
  }

  /// Removes every staged remote version except the retained set (current and
  /// previous). Returns the versions that were removed.
  pub async fn prune_remote_bundles(
    &self,
    bundle_name: String,
  ) -> Result<Vec<String>, crate::Error> {
    let removed = self.inner.prune_remote_bundles(&bundle_name).await?;
    Ok(removed)
  }
}
