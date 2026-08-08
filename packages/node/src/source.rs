use crate::bundle::Bundle;
use crate::bundle::BundleDescriptor;
use crate::bundle::BundleDescriptorInner;
use crate::bundle::{DataReadOptions, HeaderReadOptions, IndexReadOptions};
use crate::integrity::{IntegrityChecker, IntegrityPolicy};
use crate::js::JsCallbackExt;
use crate::signature::SignatureVerifier;
use napi::bindgen_prelude::*;
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
      metadata: value.item.data.into(),
    }
  }
}

/// A descriptor loaded (and cached) by a [`BundleSource`].
///
/// Holds the parsed header/index together with the filepath it was loaded from, so
/// reading entry data always targets the exact bundle version that produced this
/// descriptor — even if the source's active version is swapped concurrently.
///
/// The instance owns a reference-counted handle to the cached descriptor. When the
/// JavaScript object is garbage-collected, the handle is released automatically; the
/// underlying descriptor stays alive only while the source cache (see
/// {@link BundleSource.loadDescriptor}) or another `LoadedDescriptor` references it.
/// No manual disposal is required and no memory is leaked.
#[napi]
pub struct LoadedDescriptor {
  pub(crate) inner: Arc<source::LoadedDescriptor>,
}

#[napi]
impl LoadedDescriptor {
  /// Returns the bundle descriptor
  ///
  /// The returned descriptor shares the same in-memory metadata and carries no
  /// reference back to the source, so it can outlive this `LoadedDescriptor`.
  ///
  /// @returns {BundleDescriptor} Bundle metadata
  ///
  /// @example
  /// ```typescript
  /// const loaded = await source.loadDescriptor('app');
  /// const index = loaded.descriptor().index();
  /// console.log(index.containsPath('/index.html'));
  /// ```
  #[napi]
  pub fn descriptor(&self) -> BundleDescriptor {
    BundleDescriptor {
      inner: BundleDescriptorInner::Arc(self.inner.descriptor().clone()),
    }
  }

  /// Reads file data for `path`, loading it lazily from disk.
  ///
  /// The read targets the bundle file this descriptor was loaded from, so the data
  /// is always consistent with {@link LoadedDescriptor.descriptor} even if the
  /// source's active version changes meanwhile. Returns `null` if the path does not
  /// exist in the bundle.
  ///
  /// @param {string} path - File path in the bundle (e.g., "/index.html")
  /// @returns {Promise<Buffer | null>} File contents or null if not found
  ///
  /// @example
  /// ```typescript
  /// const loaded = await source.loadDescriptor('app');
  /// const html = await loaded.getData('/index.html');
  /// if (html) {
  ///   console.log(html.toString('utf-8'));
  /// }
  /// ```
  #[napi]
  pub async fn get_data(&self, path: String) -> crate::Result<Option<Buffer>> {
    let data = self.inner.get_data(&path).await?;
    Ok(data.map(|x| x.into()))
  }

  /// Reads the checksum of file data for `path`, loading it lazily from disk.
  ///
  /// @param {string} path - File path in the bundle
  /// @returns {Promise<number | null>} xxHash-32 checksum or null if not found
  #[napi]
  pub async fn get_data_checksum(&self, path: String) -> crate::Result<Option<u32>> {
    let checksum = self.inner.get_data_checksum(&path).await?;
    Ok(checksum)
  }
}

/// Which bundles a load-time verification applies to.
///
/// @enum {string}
#[napi(string_enum = "camelCase")]
pub enum BundleSourceVerifyMode {
  /// Verify both builtin and remote bundles. Builtin bundles ship inside the application,
  /// so the builtin manifest must carry the metadata being verified for the check to have
  /// anything to work with.
  All,
  /// Verify downloaded (remote) bundles only.
  OnlyRemote,
}

impl From<BundleSourceVerifyMode> for source::BundleSourceVerifyMode {
  fn from(value: BundleSourceVerifyMode) -> Self {
    match value {
      BundleSourceVerifyMode::All => Self::All,
      BundleSourceVerifyMode::OnlyRemote => Self::OnlyRemote,
    }
  }
}

/// How bundles are checked against the integrity recorded for them in the manifest when
/// they are loaded from disk.
///
/// @property {IntegrityPolicy} [policy] - How a bundle's integrity metadata is treated (default: 'optional'; 'off' disables the check)
/// @property {Function} [check] - Custom checker that validates bundle bytes against an integrity string
/// @property {BundleSourceVerifyMode} [checkMode] - Which bundles are checked on load (default: 'onlyRemote')
#[napi(object, object_to_js = false)]
pub struct BundleSourceIntegrityOptions {
  pub policy: Option<IntegrityPolicy>,
  #[napi(ts_type = "(data: Uint8Array, integrity: string) => Promise<boolean>")]
  pub check: Option<IntegrityChecker>,
  pub check_mode: Option<BundleSourceVerifyMode>,
}

/// How bundle signatures are verified when bundles are loaded from disk.
///
/// A bundle's signature signs its integrity string (e.g. `sha256:<base64>`), not the
/// bundle bytes; verifying it proves the integrity string is authentic. It is verified
/// independently of the integrity check, so pair it with an enabled integrity policy to
/// also authenticate the bytes — signature verification alone does not read them.
///
/// @property {SignatureVerifierOptions | Function} [verify] - Signature verification config or custom function. A custom function receives `message` — the UTF-8 bytes of the bundle's integrity string (e.g. `sha256:<base64>`), which is what the signature covers — and NOT the bundle bytes.
/// @property {BundleSourceVerifyMode} [verifyMode] - Which bundles have their signature verified on load (default: 'onlyRemote')
#[napi(object, object_to_js = false)]
pub struct BundleSourceSignatureOptions {
  #[napi(
    ts_type = "SignatureVerifierOptions | ((message: Uint8Array, signature: string) => Promise<boolean>)"
  )]
  pub verify: Option<SignatureVerifier>,
  pub verify_mode: Option<BundleSourceVerifyMode>,
}

/// Configuration for creating a bundle source.
///
/// @property {string} builtinDir - Directory containing builtin bundles
/// @property {string} remoteDir - Directory containing remote bundles
/// @property {string} [builtinManifestFilepath] - Custom manifest path for builtin
/// @property {string} [remoteManifestFilepath] - Custom manifest path for remote
/// @property {BundleSourceIntegrityOptions} [integrity] - How bundles are checked against their manifest integrity metadata on load
/// @property {BundleSourceSignatureOptions} [signature] - How bundle signatures are verified on load
/// @property {DataReadOptions} [dataReadOptions] - Verify each entry's checksum when its data is read
/// @property {HeaderReadOptions} [headerReadOptions] - Verify the header checksum when a bundle is loaded
/// @property {IndexReadOptions} [indexReadOptions] - Verify the index checksum when a bundle is loaded
///
/// @example
/// ```typescript
/// const config = {
///   builtinDir: './bundles/builtin',
///   remoteDir: './bundles/remote',
/// };
/// const source = new BundleSource(config);
/// ```
///
/// @example
/// ```typescript
/// // Require downloaded bundles to match the integrity recorded in the manifest.
/// const source = new BundleSource({
///   builtinDir: './bundles/builtin',
///   remoteDir: './bundles/remote',
///   integrity: { policy: 'strict' },
/// });
/// ```
///
/// @example
/// ```typescript
/// // Turn off data checksum verification and seed the index checksum.
/// const source = new BundleSource({
///   builtinDir: './bundles/builtin',
///   remoteDir: './bundles/remote',
///   dataReadOptions: { checksum: { verify: false } },
///   indexReadOptions: { checksum: { seed: 42 } },
/// });
/// ```
#[napi(object, object_to_js = false)]
pub struct BundleSourceConfig {
  pub builtin_dir: String,
  pub remote_dir: String,
  pub builtin_manifest_filepath: Option<String>,
  pub remote_manifest_filepath: Option<String>,
  pub integrity: Option<BundleSourceIntegrityOptions>,
  pub signature: Option<BundleSourceSignatureOptions>,
  pub data_read_options: Option<DataReadOptions>,
  pub header_read_options: Option<HeaderReadOptions>,
  pub index_read_options: Option<IndexReadOptions>,
}

fn source_options(config: &mut BundleSourceConfig) -> source::BundleSourceOptions {
  let mut options = source::BundleSourceOptions::default();
  if let Some(integrity) = config.integrity.take() {
    let mut integrity_options = source::BundleSourceIntegrityOptions::default();
    if let Some(policy) = integrity.policy {
      integrity_options = integrity_options.policy(policy.into());
    }
    if let Some(checker) = integrity.check {
      integrity_options = integrity_options.check(wvb::integrity::IntegrityCheck::Custom(
        Arc::new(move |data, integrity| {
          let buffer = Buffer::from(data);
          let integrity = integrity.to_string();
          let callback = Arc::clone(&checker);
          Box::pin(async move {
            let ret = callback
              .invoke_async((buffer, integrity).into())
              .await?
              .await?;
            Ok(ret)
          })
        }),
      ));
    }
    if let Some(mode) = integrity.check_mode {
      integrity_options = integrity_options.check_mode(mode.into());
    }
    options = options.integrity(integrity_options);
  }
  if let Some(signature) = config.signature.take() {
    let mut signature_options = source::BundleSourceSignatureOptions::default();
    if let Some(verifier) = signature.verify {
      signature_options = signature_options.verify(verifier.inner);
    }
    if let Some(mode) = signature.verify_mode {
      signature_options = signature_options.verify_mode(mode.into());
    }
    options = options.signature(signature_options);
  }
  if let Some(read) = config.data_read_options.take() {
    options = options.data_read(read.into());
  }
  if let Some(read) = config.header_read_options.take() {
    options = options.header_read(read.into());
  }
  if let Some(read) = config.index_read_options.take() {
    options = options.index_read(read.into());
  }
  options
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
///   builtinDir: './bundles/builtin',
///   remoteDir: './bundles/remote',
/// });
///
/// // List all bundles
/// const bundles = await source.listBundles();
///
/// // Load current version
/// const version = await source.loadVersion('app');
///
/// // Fetch bundle
/// const bundle = await source.fetch('app');
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
  ///   builtinDir: './builtin',
  ///   remoteDir: './remote',
  /// });
  /// ```
  #[napi(constructor)]
  pub fn new(mut config: BundleSourceConfig) -> crate::Result<BundleSource> {
    let options = source_options(&mut config);
    let mut builder = source::BundleSource::builder()
      .builtin_dir(config.builtin_dir)
      .remote_dir(config.remote_dir)
      .options(options);
    if let Some(builtin_manifest) = config.builtin_manifest_filepath {
      builder = builder.builtin_manifest_filepath(builtin_manifest);
    }
    if let Some(remote_manifest) = config.remote_manifest_filepath {
      builder = builder.remote_manifest_filepath(remote_manifest);
    }
    let source = builder.build();
    Ok(BundleSource {
      inner: Arc::new(source),
    })
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
  /// const version = await source.loadVersion('app');
  /// if (version) {
  ///   console.log(`Current version: ${version.version} (${version.type})`);
  /// }
  /// ```
  #[napi]
  pub async fn load_version(
    &self,
    bundle_name: String,
  ) -> crate::Result<Option<BundleSourceVersion>> {
    let version = self.inner.get_version(&bundle_name).await?;
    Ok(version.map(Into::into))
  }

  /// Updates the current version for a remote bundle.
  ///
  /// Changes which version is considered "current" in the manifest.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @param {string} version - Version to set as current
  ///
  /// @example
  /// ```typescript
  /// await source.updateRemoteVersion('app', '1.2.0');
  /// ```
  #[napi]
  pub async fn update_remote_version(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<()> {
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
  /// const path = await source.resolveFilepath('app');
  /// console.log(`Bundle at: ${path}`);
  /// ```
  #[napi]
  pub async fn resolve_filepath(&self, bundle_name: String) -> crate::Result<String> {
    let filepath = self.inner.resolve_filepath(&bundle_name).await?;
    Ok(filepath.to_string_lossy().to_string())
  }

  /// Get the file path for a builtin bundle.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @param {string} version - Version of the bundle
  /// @returns {string} Absolute file path
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

  /// Get the file path for a remote bundle.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @param {string} version - Version of the bundle
  /// @returns {string} Absolute file path
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

  /// Fetches a bundle.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @returns {Promise<Bundle>} Fetched bundle
  ///
  /// @example
  /// ```typescript
  /// const bundle = await source.fetchBundle('app');
  /// const html = bundle.getData('/index.html');
  /// ```
  #[napi]
  pub async fn fetch_bundle(&self, bundle_name: String) -> crate::Result<Bundle> {
    let inner = self.inner.fetch_bundle(&bundle_name).await?;
    Ok(Bundle { inner })
  }

  /// Fetches a builtin bundle.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @param {string} version - Version of the bundle
  /// @returns {Promise<Bundle>} Fetched bundle
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

  /// Fetches a remote bundle.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @param {string} version - Version of the bundle
  /// @returns {Promise<Bundle>} Fetched bundle
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

  /// Fetches only the bundle descriptor.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @returns {Promise<BundleDescriptor>} Bundle descriptor
  ///
  /// @example
  /// ```typescript
  /// const descriptor = await source.fetchDescriptor('app');
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

  /// Load builtin bundle metadata.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @param {string} version - Version of the bundle
  /// @returns {Promise<BundleManifestMetadata | null>} Loaded metadata
  #[napi]
  pub async fn load_builtin_metadata(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<Option<BundleManifestMetadata>> {
    let metadata = self
      .inner
      .get_builtin_metadata(&bundle_name, &version)
      .await?
      .map(BundleManifestMetadata::from);
    Ok(metadata)
  }

  /// Load remote bundle metadata.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @param {string} version - Version of the bundle
  /// @returns {Promise<BundleManifestMetadata | null>} Loaded metadata
  #[napi]
  pub async fn load_remote_metadata(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<Option<BundleManifestMetadata>> {
    let metadata = self
      .inner
      .get_remote_metadata(&bundle_name, &version)
      .await?
      .map(BundleManifestMetadata::from);
    Ok(metadata)
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
  /// await source.writeRemoteBundle('app', '1.2.0', bundle, {
  ///   integrity: 'sha3-384-...',
  ///   etag: 'abc123',
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

  /// Loads (and caches) the descriptor for the current version of a bundle.
  ///
  /// The descriptor reads entry data lazily from disk via
  /// {@link LoadedDescriptor.getData}, avoiding loading the full bundle into memory.
  /// Concurrent calls for the same bundle share a single load (single-flight) and
  /// return the cached descriptor until the active version changes or
  /// {@link BundleSource.unloadDescriptor} is called.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @returns {Promise<LoadedDescriptor>} Loaded descriptor
  ///
  /// @example
  /// ```typescript
  /// const loaded = await source.loadDescriptor('app');
  /// const html = await loaded.getData('/index.html');
  /// ```
  #[napi]
  pub async fn load_descriptor(&self, bundle_name: String) -> crate::Result<LoadedDescriptor> {
    let inner = self.inner.load(&bundle_name).await?;
    Ok(LoadedDescriptor { inner })
  }

  /// Drops the cached descriptor for a bundle, if present.
  ///
  /// Already-returned {@link LoadedDescriptor} handles keep working; they hold their
  /// own reference and are unaffected. The next {@link BundleSource.loadDescriptor}
  /// reloads from disk.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @returns {boolean} True if a cached descriptor was removed
  #[napi]
  pub fn unload_descriptor(&self, bundle_name: String) -> bool {
    self.inner.unload(&bundle_name)
  }

  /// Removes a single staged remote bundle version.
  ///
  /// Drops its manifest entry and deletes its file from disk.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @param {string} version - Version to remove
  /// @returns {Promise<boolean>} True if the entry existed and was removed
  #[napi]
  pub async fn remove_remote_bundle(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<bool> {
    let removed = self
      .inner
      .remove_remote_bundle(&bundle_name, &version)
      .await?;
    Ok(removed)
  }

  /// Returns the remote versions that pruning retains (the current and previous versions).
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @returns {Promise<string[]>} Retained version strings
  #[napi]
  pub async fn remote_retained_versions(&self, bundle_name: String) -> crate::Result<Vec<String>> {
    let versions = self.inner.remote_retained_versions(&bundle_name).await?;
    Ok(versions)
  }

  /// Removes every staged remote version except the retained set (current and previous).
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @returns {Promise<string[]>} Versions that were removed
  #[napi]
  pub async fn prune_remote_bundles(&self, bundle_name: String) -> crate::Result<Vec<String>> {
    let removed = self.inner.prune_remote_bundles(&bundle_name).await?;
    Ok(removed)
  }
}
