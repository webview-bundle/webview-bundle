use crate::bundle::Bundle;
use crate::bundle::BundleDescriptor;
use crate::bundle::BundleDescriptorInner;
use napi_derive::napi;
use std::collections::HashMap;
use std::sync::Arc;
use wvb::source;

/// The type of bundle source: builtin or remote.
///
/// @enum {string}
#[napi(string_enum = "lowercase")]
pub enum BundleSourceKind {
  /// Bundles shipped with the application (read-only, fallback)
  Builtin,
  /// Downloaded bundles (takes priority over builtin)
  Remote,
}

impl From<source::BundleSourceKind> for BundleSourceKind {
  fn from(value: source::BundleSourceKind) -> Self {
    match value {
      source::BundleSourceKind::Builtin => Self::Builtin,
      source::BundleSourceKind::Remote => Self::Remote,
    }
  }
}

/// Bundle version with source kind information.
///
/// Indicates which source (builtin or remote) provides a bundle version.
///
/// @property {BundleSourceKind} type - The source kind
/// @property {string} version - The version string (e.g., "1.0.0")
#[napi(object)]
pub struct BundleSourceVersion {
  #[napi(js_name = "type")]
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

/// Metadata for a bundle version in the manifest.
///
/// Contains cache validation and integrity information.
///
/// @property {string} [etag] - HTTP ETag for cache validation
/// @property {string} [integrity] - SHA3 integrity hash for verification
/// @property {string} [signature] - Digital signature for authentication
/// @property {string} [lastModified] - HTTP Last-Modified timestamp
#[napi(object)]
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

/// Manifest format version.
///
/// @enum {number}
#[napi]
pub enum BundleManifestVersion {
  V1 = 1,
}

/// Entry for a single bundle in the manifest.
///
/// Contains all versions and the current active version.
///
/// @property {Record<string, BundleManifestMetadata>} versions - Available versions
/// @property {string} currentVersion - Currently active version
#[napi(object)]
pub struct BundleManifestEntry {
  pub versions: HashMap<String, BundleManifestMetadata>,
  pub current_version: String,
}

/// Complete manifest data structure.
///
/// The manifest tracks all bundle versions and metadata.
///
/// @property {1} manifestVersion - Manifest format version (always 1)
/// @property {Record<string, BundleManifestEntry>} entries - Bundle entries by name
#[napi(object)]
pub struct BundleManifestData {
  #[napi(ts_type = "1")]
  pub manifest_version: BundleManifestVersion,
  pub entries: HashMap<String, BundleManifestEntry>,
}

/// Information about a bundle from list operations.
///
/// @property {BundleSourceKind} type - Source kind (builtin or remote)
/// @property {string} name - Bundle name
/// @property {string} version - Version string
/// @property {boolean} current - Whether this is the current active version
/// @property {BundleManifestMetadata} metadata - Bundle metadata
#[napi(object)]
pub struct ListBundleItem {
  #[napi(js_name = "type")]
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

/// Configuration for creating a bundle source.
///
/// @property {string} builtinDir - Directory containing builtin bundles
/// @property {string} remoteDir - Directory containing remote bundles
/// @property {string} [builtinManifestFilepath] - Custom manifest path for builtin
/// @property {string} [remoteManifestFilepath] - Custom manifest path for remote
///
/// @example
/// ```typescript
/// const config = {
///   builtinDir: "./bundles/builtin",
///   remoteDir: "./bundles/remote"
/// };
/// const source = new BundleSource(config);
/// ```
#[napi(object)]
pub struct BundleSourceConfig {
  pub builtin_dir: String,
  pub remote_dir: String,
  pub builtin_manifest_filepath: Option<String>,
  pub remote_manifest_filepath: Option<String>,
}

/// Bundle source for managing multiple bundle versions.
///
/// A source manages bundles in two directories:
/// - **builtin**: Bundles shipped with the app (read-only, fallback)
/// - **remote**: Downloaded bundles (takes priority)
///
/// The source automatically handles version selection, with remote bundles
/// taking priority over builtin ones.
///
/// @example
/// ```typescript
/// const source = new BundleSource({
///   builtinDir: "./bundles/builtin",
///   remoteDir: "./bundles/remote"
/// });
///
/// // List all bundles
/// const bundles = await source.listBundles();
///
/// // Load current version
/// const version = await source.loadVersion("app");
///
/// // Fetch bundle
/// const bundle = await source.fetch("app");
/// ```
#[napi]
pub struct BundleSource {
  pub(crate) inner: Arc<source::BundleSource>,
}

#[napi]
impl BundleSource {
  /// Creates a new bundle source.
  ///
  /// @param {BundleSourceConfig} config - Source configuration
  ///
  /// @example
  /// ```typescript
  /// const source = new BundleSource({
  ///   builtinDir: "./builtin",
  ///   remoteDir: "./remote"
  /// });
  /// ```
  #[napi(constructor)]
  pub fn new(config: BundleSourceConfig) -> BundleSource {
    let mut builder = source::BundleSource::builder()
      .builtin_dir(config.builtin_dir)
      .remote_dir(config.remote_dir);
    if let Some(builtin_manifest) = config.builtin_manifest_filepath {
      builder = builder.builtin_manifest_filepath(builtin_manifest);
    }
    if let Some(remote_manifest) = config.remote_manifest_filepath {
      builder = builder.remote_manifest_filepath(remote_manifest);
    }
    let source = builder.build();
    BundleSource {
      inner: Arc::new(source),
    }
  }

  /// Lists all available bundles from both sources.
  ///
  /// Returns bundles from both builtin and remote directories, including
  /// all versions and metadata.
  ///
  /// @returns {Promise<ListBundleItem[]>} List of bundle items
  ///
  /// @example
  /// ```typescript
  /// const bundles = await source.listBundles();
  /// for (const bundle of bundles) {
  ///   console.log(`${bundle.name}@${bundle.version} (${bundle.type})`);
  /// }
  /// ```
  #[napi]
  pub async fn list_bundles(&self) -> crate::Result<Vec<ListBundleItem>> {
    let items = self
      .inner
      .list_bundles()
      .await?
      .into_iter()
      .map(ListBundleItem::from)
      .collect::<Vec<_>>();
    Ok(items)
  }

  /// Loads the current version for a bundle.
  ///
  /// Returns the version from remote if available, otherwise from builtin.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @returns {Promise<BundleSourceVersion | null>} Version info or null if not found
  ///
  /// @example
  /// ```typescript
  /// const version = await source.loadVersion("app");
  /// if (version) {
  ///   console.log(`Current version: ${version.version} (${version.type})`);
  /// }
  /// ```
  #[napi]
  pub async fn load_version(
    &self,
    bundle_name: String,
  ) -> crate::Result<Option<BundleSourceVersion>> {
    let version = self.inner.load_version(&bundle_name).await?;
    Ok(version.map(Into::into))
  }

  /// Updates the current version for a bundle.
  ///
  /// Changes which version is considered "current" in the manifest.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @param {string} version - Version to set as current
  ///
  /// @example
  /// ```typescript
  /// await source.updateVersion("app", "1.2.0");
  /// ```
  #[napi]
  pub async fn update_version(&self, bundle_name: String, version: String) -> crate::Result<()> {
    self
      .inner
      .update_remote_version(&bundle_name, &version)
      .await?;
    Ok(())
  }

  /// Gets the file path for a bundle.
  ///
  /// Returns the path to the `.wvb` file for the current version,
  /// preferring remote over builtin.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @returns {Promise<string>} Absolute file path
  ///
  /// @example
  /// ```typescript
  /// const path = await source.filepath("app");
  /// console.log(`Bundle at: ${path}`);
  /// ```
  #[napi]
  pub async fn filepath(&self, bundle_name: String) -> crate::Result<String> {
    let filepath = self.inner.filepath(&bundle_name).await?;
    Ok(filepath.to_string_lossy().to_string())
  }

  /// Fetches and loads a bundle.
  ///
  /// Loads the entire bundle into memory for the current version.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @returns {Promise<Bundle>} Loaded bundle
  ///
  /// @example
  /// ```typescript
  /// const bundle = await source.fetch("app");
  /// const html = bundle.getData("/index.html");
  /// ```
  #[napi]
  pub async fn fetch(&self, bundle_name: String) -> crate::Result<Bundle> {
    let inner = self.inner.fetch(&bundle_name).await?;
    Ok(Bundle { inner })
  }

  /// Fetches only the bundle descriptor (metadata).
  ///
  /// Loads only header and index without file data, useful for inspection.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @returns {Promise<BundleDescriptor>} Bundle descriptor
  ///
  /// @example
  /// ```typescript
  /// const descriptor = await source.fetchDescriptor("app");
  /// const index = descriptor.index();
  /// console.log(`Files: ${Object.keys(index.entries()).length}`);
  /// ```
  #[napi]
  pub async fn fetch_descriptor(&self, bundle_name: String) -> crate::Result<BundleDescriptor> {
    let inner = self.inner.fetch_descriptor(&bundle_name).await?;
    Ok(BundleDescriptor {
      inner: BundleDescriptorInner::Owned(inner),
    })
  }

  /// Writes a bundle to the remote directory.
  ///
  /// Installs a new bundle version to the remote directory and updates
  /// the manifest.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @param {string} version - Version string
  /// @param {Bundle} bundle - Bundle to write
  /// @param {BundleManifestMetadata} metadata - Bundle metadata
  ///
  /// @example
  /// ```typescript
  /// await source.writeRemoteBundle("app", "1.2.0", bundle, {
  ///   integrity: "sha3-384-...",
  ///   etag: "abc123"
  /// });
  /// ```
  #[napi]
  pub async fn write_remote_bundle(
    &self,
    bundle_name: String,
    version: String,
    bundle: &Bundle,
    metadata: BundleManifestMetadata,
  ) -> crate::Result<()> {
    self
      .inner
      .write_remote_bundle(&bundle_name, &version, &bundle.inner, metadata.into())
      .await?;
    Ok(())
  }
}
