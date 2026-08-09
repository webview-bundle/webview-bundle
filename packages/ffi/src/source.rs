use crate::bundle::{
  Bundle, BundleDescriptor, BundleDescriptorInner, DataReadOptions, HeaderReadOptions,
  IndexReadOptions,
};
use crate::integrity::IntegrityPolicy;
use crate::signature::SignatureVerification;
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

impl From<source::BundleManifestVersionData> for BundleManifestMetadata {
  fn from(value: source::BundleManifestVersionData) -> Self {
    Self {
      etag: value.etag,
      integrity: value.integrity,
      signature: value.signature,
      last_modified: value.last_modified,
    }
  }
}

impl From<BundleManifestMetadata> for source::BundleManifestVersionData {
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
      metadata: value.item.data.into(),
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

#[derive(uniffi::Record, Clone)]
pub struct BundleSourceConfig {
  pub builtin_dir: String,
  pub remote_dir: String,
  #[uniffi(default = None)]
  pub builtin_manifest_filepath: Option<String>,
  #[uniffi(default = None)]
  pub remote_manifest_filepath: Option<String>,
  /// How bundles are checked against their manifest integrity metadata on load.
  #[uniffi(default = None)]
  pub integrity: Option<BundleSourceIntegrityOptions>,
  /// How bundle signatures are verified on load.
  #[uniffi(default = None)]
  pub signature: Option<BundleSourceSignatureOptions>,
  /// How each entry's checksum is verified when its data is read.
  #[uniffi(default = None)]
  pub data_read_options: Option<DataReadOptions>,
  /// How a bundle's header checksum is verified when its descriptor is read on load.
  #[uniffi(default = None)]
  pub header_read_options: Option<HeaderReadOptions>,
  /// How a bundle's index checksum is verified when its descriptor is read on load.
  #[uniffi(default = None)]
  pub index_read_options: Option<IndexReadOptions>,
}

/// Which bundles a load-time verification applies to
#[derive(uniffi::Enum, Clone, Debug)]
pub enum BundleSourceVerifyMode {
  /// Verify both builtin and remote bundles.
  ///
  /// Builtin bundles ship inside the application, so the builtin manifest must carry the
  /// metadata being verified for the check to have anything to work with.
  All,
  /// Verify downloaded (remote) bundles only. This is the default.
  OnlyRemote,
}

impl From<BundleSourceVerifyMode> for source::BundleSourceVerifyMode {
  fn from(v: BundleSourceVerifyMode) -> Self {
    match v {
      BundleSourceVerifyMode::All => source::BundleSourceVerifyMode::All,
      BundleSourceVerifyMode::OnlyRemote => source::BundleSourceVerifyMode::OnlyRemote,
    }
  }
}

/// How bundles are checked against the integrity recorded for them in the manifest when
/// they are loaded from disk.
#[derive(uniffi::Record, Clone)]
pub struct BundleSourceIntegrityOptions {
  /// How a bundle's integrity metadata is treated (default: [`IntegrityPolicy::Optional`]).
  ///
  /// [`IntegrityPolicy::Off`] disables the integrity check entirely.
  #[uniffi(default = None)]
  pub policy: Option<IntegrityPolicy>,
  /// A custom checker that validates bundle bytes against their integrity string
  /// (default: the built-in checker, which compares the advertised hash).
  #[uniffi(default = None)]
  pub check: Option<Arc<dyn crate::integrity::IntegrityCheck>>,
  /// Which bundles are checked on load (default: [`BundleSourceVerifyMode::OnlyRemote`]).
  #[uniffi(default = None)]
  pub check_mode: Option<BundleSourceVerifyMode>,
}

/// How bundle signatures are verified when bundles are loaded from disk.
///
/// A bundle's signature signs its integrity string (e.g. `sha256:<base64>`), not the
/// bundle bytes; verifying it proves the integrity string is authentic. It is verified
/// independently of the integrity check, so pair it with an enabled
/// [`BundleSourceConfig::integrity`] to also authenticate the bytes — signature
/// verification alone does not read them.
#[derive(uniffi::Record, Clone)]
pub struct BundleSourceSignatureOptions {
  /// Verifies that a bundle's integrity string was signed by the matching key — with a
  /// declarative public key or a custom function (default: off).
  #[uniffi(default = None)]
  pub verify: Option<SignatureVerification>,
  /// Which bundles have their signature verified on load
  /// (default: [`BundleSourceVerifyMode::OnlyRemote`]).
  #[uniffi(default = None)]
  pub verify_mode: Option<BundleSourceVerifyMode>,
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
  config: &mut BundleSourceConfig,
) -> Result<source::BundleSourceOptions, crate::Error> {
  let mut source_options = source::BundleSourceOptions::default();
  if let Some(integrity) = config.integrity.take() {
    let mut integrity_options = source::BundleSourceIntegrityOptions::default();
    if let Some(policy) = integrity.policy {
      integrity_options = integrity_options.policy(policy.into());
    }
    if let Some(check) = integrity.check {
      integrity_options = integrity_options.check(crate::integrity::into_checker(check));
    }
    if let Some(mode) = integrity.check_mode {
      integrity_options = integrity_options.check_mode(mode.into());
    }
    source_options = source_options.integrity(integrity_options);
  }
  if let Some(sig) = config.signature.take() {
    let mut signature_options = source::BundleSourceSignatureOptions::default();
    if let Some(verification) = sig.verify {
      let verifier = signature::SignatureVerify::try_from(verification)?;
      signature_options = signature_options.verify(verifier);
    }
    if let Some(mode) = sig.verify_mode {
      signature_options = signature_options.verify_mode(mode.into());
    }
    source_options = source_options.signature(signature_options);
  }
  if let Some(read_options) = config.data_read_options.take() {
    source_options = source_options.data_read(read_options.into());
  }
  if let Some(read_options) = config.header_read_options.take() {
    source_options = source_options.header_read(read_options.into());
  }
  if let Some(read_options) = config.index_read_options.take() {
    source_options = source_options.index_read(read_options.into());
  }
  Ok(source_options)
}

#[uniffi::export]
impl BundleSource {
  #[uniffi::constructor]
  pub fn new(mut config: BundleSourceConfig) -> Result<Arc<BundleSource>, crate::Error> {
    let options = source_options(&mut config)?;
    let builder = source_builder(config).options(options);
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
    self.inner.unload(&bundle_name)
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
    let version = self.inner.get_version(&bundle_name).await?;
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
    let inner = self.inner.load(&bundle_name).await?;
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
      .get_builtin_metadata(&bundle_name, &version)
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
      .get_remote_metadata(&bundle_name, &version)
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
    let removed = self.inner.prune_remote_bundle(&bundle_name).await?;
    Ok(removed)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::bundle::ChecksumReadOptions;
  use wvb::integrity;

  fn make_config(
    integrity: Option<BundleSourceIntegrityOptions>,
    signature: Option<BundleSourceSignatureOptions>,
    data_read_options: Option<DataReadOptions>,
    header_read_options: Option<HeaderReadOptions>,
    index_read_options: Option<IndexReadOptions>,
  ) -> BundleSourceConfig {
    BundleSourceConfig {
      builtin_dir: String::new(),
      remote_dir: String::new(),
      builtin_manifest_filepath: None,
      remote_manifest_filepath: None,
      integrity,
      signature,
      data_read_options,
      header_read_options,
      index_read_options,
    }
  }

  #[test]
  fn source_options_maps_nested_records() {
    let mut config = make_config(
      Some(BundleSourceIntegrityOptions {
        policy: Some(IntegrityPolicy::Strict),
        check: None,
        check_mode: Some(BundleSourceVerifyMode::All),
      }),
      Some(BundleSourceSignatureOptions {
        verify: None,
        verify_mode: Some(BundleSourceVerifyMode::OnlyRemote),
      }),
      Some(DataReadOptions {
        checksum: Some(ChecksumReadOptions {
          verify: Some(false),
          seed: Some(42),
        }),
      }),
      Some(HeaderReadOptions {
        checksum: Some(ChecksumReadOptions {
          verify: Some(true),
          seed: Some(7),
        }),
      }),
      None,
    );

    let expected = source::BundleSourceOptions::default()
      .integrity(
        source::BundleSourceIntegrityOptions::default()
          .policy(integrity::IntegrityPolicy::Strict)
          .check_mode(source::BundleSourceVerifyMode::All),
      )
      .signature(
        source::BundleSourceSignatureOptions::default()
          .verify_mode(source::BundleSourceVerifyMode::OnlyRemote),
      )
      .data_read(
        wvb::DataReadOptions::default()
          .checksum(wvb::ChecksumReadOptions::default().verify(false).seed(42)),
      )
      .header_read(
        wvb::HeaderReadOptions::default()
          .checksum(wvb::ChecksumReadOptions::default().verify(true).seed(7)),
      );

    assert_eq!(
      format!("{:?}", source_options(&mut config).unwrap()),
      format!("{expected:?}"),
    );
  }

  #[test]
  fn source_options_maps_a_custom_integrity_check() {
    struct AlwaysValid;
    #[async_trait::async_trait]
    impl crate::integrity::IntegrityCheck for AlwaysValid {
      async fn check(&self, _data: Vec<u8>, _integrity: String) -> bool {
        true
      }
    }

    let mut config = make_config(
      Some(BundleSourceIntegrityOptions {
        policy: None,
        check: Some(Arc::new(AlwaysValid)),
        check_mode: None,
      }),
      None,
      None,
      None,
      None,
    );

    let mapped = source_options(&mut config).unwrap();
    assert!(format!("{mapped:?}").contains("IntegrityChecker::Custom"));
  }

  #[test]
  fn source_options_all_none_equals_default() {
    let mut config = make_config(None, None, None, None, None);

    assert_eq!(
      format!("{:?}", source_options(&mut config).unwrap()),
      format!("{:?}", source::BundleSourceOptions::default()),
    );
  }
}
