use crate::bundle::{Bundle, BundleDescriptor, BundleDescriptorInner};
use std::collections::HashMap;
use std::sync::Arc;
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

/// Unified access point for bundles from both the builtin and remote sources.
///
/// The remote source takes precedence over the builtin source when both contain
/// a bundle with the same name.
#[derive(uniffi::Object)]
pub struct BundleSource {
  pub(crate) inner: Arc<source::BundleSource>,
}

#[uniffi::export]
impl BundleSource {
  #[uniffi::constructor]
  pub fn new(config: BundleSourceConfig) -> Arc<BundleSource> {
    let mut builder = source::BundleSource::builder()
      .builtin_dir(config.builtin_dir)
      .remote_dir(config.remote_dir);
    if let Some(p) = config.builtin_manifest_filepath {
      builder = builder.builtin_manifest_filepath(p);
    }
    if let Some(p) = config.remote_manifest_filepath {
      builder = builder.remote_manifest_filepath(p);
    }
    Arc::new(BundleSource {
      inner: Arc::new(builder.build()),
    })
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

  pub async fn filepath(&self, bundle_name: String) -> Result<String, crate::Error> {
    let path = self.inner.bundle_filepath(&bundle_name).await?;
    Ok(path.to_string_lossy().to_string())
  }

  /// Loads the full bundle (header + index + data) for `bundle_name`.
  pub async fn fetch(&self, bundle_name: String) -> Result<Arc<Bundle>, crate::Error> {
    let inner = self.inner.fetch(&bundle_name).await?;
    Ok(Arc::new(Bundle {
      inner: Arc::new(inner),
    }))
  }

  /// Loads only the header and index for `bundle_name`, skipping the data section.
  /// The returned descriptor does not support [`BundleDescriptor::index`]; use
  /// [`fetch`](BundleSource::fetch) when entry data is needed.
  pub async fn fetch_descriptor(
    &self,
    bundle_name: String,
  ) -> Result<Arc<BundleDescriptor>, crate::Error> {
    let inner = self.inner.fetch_descriptor(&bundle_name).await?;
    Ok(Arc::new(BundleDescriptor {
      inner: BundleDescriptorInner::Owned(inner),
    }))
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

  pub fn bundles_map(&self) -> HashMap<String, Vec<ListBundleItem>> {
    HashMap::new()
  }
}
