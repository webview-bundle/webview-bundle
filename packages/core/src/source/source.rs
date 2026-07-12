#[cfg(feature = "integrity")]
use crate::integrity::{IntegrityChecker, IntegrityPolicy};
#[cfg(feature = "signature")]
use crate::signature::SignatureVerifier;
use crate::source::{
  BundleManifest, BundleManifestMetadata, ListBundleManifestItem, ReadOnly, ReadWrite, utils,
};
#[cfg(feature = "integrity")]
use crate::verify::VerifyOptions;
use crate::{
  AsyncBundleReader, AsyncReader, Bundle, BundleDescriptor, BundleReader, DataReadOptions,
  EXTENSION, MANIFEST_FILENAME, Reader, Writer,
};
use dashmap::DashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::OnceCell;

/// The type of bundle source: builtin or remote.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub enum BundleSourceKind {
  /// Bundles shipped with the application (read-only, fallback)
  Builtin,
  /// Downloaded bundles (takes priority)
  Remote,
}

/// Bundle version with source kind information.
///
/// This indicates which source (builtin or remote) provides a bundle version.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct BundleSourceVersion {
  /// The source kind (builtin or remote)
  #[cfg_attr(feature = "_serde", serde(rename = "type"))]
  pub kind: BundleSourceKind,
  /// The version string (e.g., "1.0.0")
  pub version: String,
}

impl BundleSourceVersion {
  /// Creates a new bundle source version.
  pub fn new(kind: BundleSourceKind, version: String) -> Self {
    Self { kind, version }
  }

  /// Creates a builtin source version.
  pub fn builtin(version: String) -> Self {
    Self::new(BundleSourceKind::Builtin, version)
  }

  /// Creates a remote source version.
  pub fn remote(version: String) -> Self {
    Self::new(BundleSourceKind::Remote, version)
  }
}

/// Which bundles are verified against their manifest metadata when loaded from disk.
///
/// A bundle is verified once per version, when it is first read; the result is cached
/// along with the descriptor, so serving a bundle does not re-hash it on every request.
#[cfg(feature = "integrity")]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub enum VerifyOnLoad {
  /// Never verify on load. Bundles are still verified when downloaded and installed.
  #[default]
  None,
  /// Verify downloaded (remote) bundles only.
  ///
  /// Builtin bundles ship inside the application and only carry integrity metadata if the
  /// app was packed with it, so this is the setting that works without changes to how
  /// builtin bundles are built.
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

/// How a [`BundleSource`] verifies bundles it reads from disk.
///
/// Two independent layers:
///
/// - **Load-time integrity/signature** (`verify_on_load`): hashes the whole bundle file and
///   checks it against the integrity (and signature) recorded in the manifest. Paid once
///   per bundle version. Detects a file damaged or replaced since it was installed.
///   Off by default; requires the `integrity` feature.
/// - **Read-time data checksum** ([`DataReadOptions`]): checks each entry's xxHash-32 as it
///   is read. Cheap, per read, and catches corruption that happens after the bundle was
///   loaded. On by default.
///
/// The checksum is a corruption detector, not a security control — its seed is public, so
/// whatever can rewrite an entry can rewrite its checksum. Only a signature detects
/// deliberate tampering, because only it cannot be recomputed without the signing key.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct BundleSourceOptions {
  #[cfg(feature = "integrity")]
  pub(crate) verify_on_load: VerifyOnLoad,
  pub(crate) data: DataReadOptions,
  #[cfg(feature = "integrity")]
  pub(crate) verify: VerifyOptions,
}

impl Default for BundleSourceOptions {
  fn default() -> Self {
    Self {
      #[cfg(feature = "integrity")]
      verify_on_load: VerifyOnLoad::default(),
      // Entry data read through a source is verified by default, matching
      // `BundleProtocol`. The `DataReadOptions` default stays off because it also serves
      // `Bundle::get_data`, which has no way to learn the seed the bundle was built with.
      data: DataReadOptions::new().verify_checksum(true),
      #[cfg(feature = "integrity")]
      verify: VerifyOptions::default(),
    }
  }
}

impl BundleSourceOptions {
  pub fn new() -> Self {
    Self::default()
  }

  /// Which bundles are verified against their manifest metadata when loaded
  /// (default: [`VerifyOnLoad::None`]).
  #[cfg(feature = "integrity")]
  pub fn verify_on_load(mut self, verify: VerifyOnLoad) -> Self {
    self.verify_on_load = verify;
    self
  }

  /// How entry data read through this source is checked (default: verified, with seed `0`).
  ///
  /// Replaces the options wholesale. Prefer [`BundleSourceOptions::verify_data_checksum`] and
  /// [`BundleSourceOptions::data_checksum_seed`] to override one field without resetting the
  /// other back to the [`DataReadOptions`] default, which is *not* the default used here.
  ///
  /// `BundleProtocol` (the `protocol` feature) overrides this with its own options.
  pub fn data(mut self, options: DataReadOptions) -> Self {
    self.data = options;
    self
  }

  /// Verifies each entry's checksum when its data is read through this source
  /// (default: `true`).
  pub fn verify_data_checksum(mut self, verify: bool) -> Self {
    self.data = self.data.verify_checksum(verify);
    self
  }

  /// The seed this source's bundles had their data checksums built with (default: `0`).
  pub fn data_checksum_seed(mut self, seed: u32) -> Self {
    self.data = self.data.checksum_seed(seed);
    self
  }

  #[cfg(feature = "integrity")]
  pub fn integrity_checker(mut self, checker: IntegrityChecker) -> Self {
    self.verify.set_integrity_checker(checker);
    self
  }

  #[cfg(feature = "integrity")]
  pub fn integrity_policy(mut self, policy: IntegrityPolicy) -> Self {
    self.verify.set_integrity_policy(policy);
    self
  }

  /// Verifies that a bundle's integrity string was signed by the matching key.
  ///
  /// The signature signs the integrity string, so configuring a verifier also makes the
  /// integrity check mandatory regardless of [`BundleSourceOptions::integrity_policy`] — a
  /// signature over an unchecked hash proves nothing about the bundle's bytes.
  ///
  /// **A verifier alone verifies nothing.** Load-time verification only runs when
  /// [`BundleSourceOptions::verify_on_load`] selects the bundles to verify, which it does
  /// not by default; set it to [`VerifyOnLoad::Remote`] (or [`VerifyOnLoad::All`]) as well.
  #[cfg(feature = "signature")]
  pub fn signature_verifier(mut self, verifier: SignatureVerifier) -> Self {
    self.verify.set_signature_verifier(verifier);
    self
  }
}

/// Builder for creating a `BundleSource`.
///
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "source")]
/// # {
/// use wvb::source::BundleSource;
///
/// let source = BundleSource::builder()
///     .builtin_dir("./builtin")
///     .remote_dir("./remote")
///     .build();
/// # }
/// ```
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct BundleSourceBuilder {
  builtin_dir: PathBuf,
  builtin_manifest_filepath: Option<PathBuf>,
  remote_dir: PathBuf,
  remote_manifest_filepath: Option<PathBuf>,
  options: BundleSourceOptions,
}

impl BundleSourceBuilder {
  pub fn new() -> Self {
    Self::default()
  }

  #[must_use]
  pub fn builtin_dir(mut self, dir: impl Into<PathBuf>) -> Self {
    self.builtin_dir = dir.into();
    self
  }

  pub fn builtin_manifest_filepath(mut self, filepath: impl Into<PathBuf>) -> Self {
    self.builtin_manifest_filepath = Some(filepath.into());
    self
  }

  #[must_use]
  pub fn remote_dir(mut self, dir: impl Into<PathBuf>) -> Self {
    self.remote_dir = dir.into();
    self
  }

  pub fn remote_manifest_filepath(mut self, filepath: impl Into<PathBuf>) -> Self {
    self.remote_manifest_filepath = Some(filepath.into());
    self
  }

  /// How bundles read through this source are verified.
  ///
  /// By default entry checksums are verified as data is read, while load-time integrity and
  /// signature verification is off — see [`BundleSourceOptions`].
  #[must_use]
  pub fn options(mut self, options: BundleSourceOptions) -> Self {
    self.options = options;
    self
  }

  pub fn build(self) -> BundleSource {
    let builtin_dir = self.builtin_dir;
    let builtin_manifest_filepath = self
      .builtin_manifest_filepath
      .map(|x| utils::normalize_path(&builtin_dir, &x))
      .unwrap_or(builtin_dir.join(MANIFEST_FILENAME));
    let remote_dir = self.remote_dir;
    let remote_manifest_filepath = self
      .remote_manifest_filepath
      .map(|x| utils::normalize_path(&remote_dir, &x))
      .unwrap_or(remote_dir.join(MANIFEST_FILENAME));
    BundleSource {
      builtin_dir,
      builtin_manifest: BundleManifest::new(&builtin_manifest_filepath, ReadOnly),
      remote_dir,
      remote_manifest: BundleManifest::new(&remote_manifest_filepath, ReadWrite),
      descriptors: DashMap::default(),
      options: self.options,
    }
  }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub struct ListBundleItem {
  #[cfg_attr(feature = "_serde", serde(rename = "type"))]
  pub kind: BundleSourceKind,
  pub item: ListBundleManifestItem,
}

/// A lazily-initialized descriptor cell, shared so concurrent loads single-flight.
type DescriptorCell = Arc<OnceCell<Arc<BundleDescriptor>>>;

#[derive(Debug)]
pub struct BundleSource {
  builtin_dir: PathBuf,
  builtin_manifest: BundleManifest<ReadOnly>,
  remote_dir: PathBuf,
  remote_manifest: BundleManifest<ReadWrite>,
  // Each entry pairs the descriptor cell with the filepath it was loaded from.
  // The filepath acts as a version fingerprint: when the active version swaps,
  // `filepath()` resolves to a different path, so `load_descriptor` notices the
  // stale entry and rebuilds. The returned `LoadedDescriptor` carries this same
  // filepath, so its `reader()` always opens the file matching the descriptor.
  descriptors: DashMap<String, (PathBuf, DescriptorCell)>,
  options: BundleSourceOptions,
}

/// A descriptor together with the filepath it was loaded from.
///
/// Holding the source filepath alongside the parsed descriptor guarantees that the
/// reader opened via [`LoadedDescriptor::reader`] always corresponds to the same
/// bundle version as the descriptor — even if the active version is swapped
/// concurrently mid-request. Dereferences to [`BundleDescriptor`].
#[derive(Debug)]
pub struct LoadedDescriptor {
  descriptor: Arc<BundleDescriptor>,
  filepath: PathBuf,
  data_options: DataReadOptions,
}

impl LoadedDescriptor {
  pub async fn reader(&self) -> crate::Result<File> {
    open_file(&self.filepath).await
  }

  pub fn descriptor(&self) -> &Arc<BundleDescriptor> {
    &self.descriptor
  }

  /// The read options this descriptor's source was configured with.
  pub fn data_options(&self) -> DataReadOptions {
    self.data_options
  }

  /// Reads the data for `path`, lazily from the bundle file this descriptor was loaded
  /// from, applying the source's [`DataReadOptions`].
  ///
  /// Returns `None` if the path doesn't exist in the bundle.
  pub async fn get_data(&self, path: &str) -> crate::Result<Option<Vec<u8>>> {
    self.get_data_with_options(path, self.data_options).await
  }

  /// Reads the data for `path` with explicit read options, overriding the source's.
  ///
  /// `BundleProtocol` (the `protocol` feature) uses this to apply its own checksum options.
  pub async fn get_data_with_options(
    &self,
    path: &str,
    options: DataReadOptions,
  ) -> crate::Result<Option<Vec<u8>>> {
    let reader = self.reader().await?;
    self
      .descriptor
      .async_get_data_with_options(reader, path, options)
      .await
  }

  /// Reads the stored checksum of the data for `path`.
  ///
  /// Returns `None` if the path doesn't exist in the bundle.
  pub async fn get_data_checksum(&self, path: &str) -> crate::Result<Option<u32>> {
    let reader = self.reader().await?;
    self.descriptor.async_get_data_checksum(reader, path).await
  }
}

impl std::ops::Deref for LoadedDescriptor {
  type Target = BundleDescriptor;

  fn deref(&self) -> &Self::Target {
    self.descriptor.as_ref()
  }
}

static WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

impl BundleSource {
  pub fn builder() -> BundleSourceBuilder {
    BundleSourceBuilder::new()
  }

  pub async fn list_bundles(&self) -> crate::Result<Vec<ListBundleItem>> {
    let (builtin_entries, remote_entries) = tokio::try_join!(
      self.builtin_manifest.list_entries(),
      self.remote_manifest.list_entries()
    )?;
    let builtin_items = builtin_entries
      .into_iter()
      .map(|item| ListBundleItem {
        kind: BundleSourceKind::Builtin,
        item,
      })
      .collect::<Vec<_>>();
    let remote_items = remote_entries
      .into_iter()
      .map(|item| ListBundleItem {
        kind: BundleSourceKind::Remote,
        item,
      })
      .collect::<Vec<_>>();
    Ok([builtin_items, remote_items].concat())
  }

  pub async fn load_version(
    &self,
    bundle_name: &str,
  ) -> crate::Result<Option<BundleSourceVersion>> {
    match self
      .remote_manifest
      .load_current_version(bundle_name)
      .await?
    {
      Some(ver) => Ok(Some(BundleSourceVersion::remote(ver))),
      None => {
        // fallback to builtin version
        let builtin_version = self
          .builtin_manifest
          .load_current_version(bundle_name)
          .await?
          .map(BundleSourceVersion::builtin);
        Ok(builtin_version)
      }
    }
  }

  pub async fn update_remote_version(&self, bundle_name: &str, version: &str) -> crate::Result<()> {
    self
      .remote_manifest
      .update_current_version(bundle_name, version)
      .await?;
    self.remote_manifest.save().await?;
    Ok(())
  }

  pub async fn resolve_filepath(&self, bundle_name: &str) -> crate::Result<PathBuf> {
    let ver = self.resolve_version(bundle_name).await?;
    self.filepath_for(bundle_name, &ver)
  }

  /// The read options this source applies to entry data (see [`BundleSourceOptions::data`]).
  pub fn data_options(&self) -> DataReadOptions {
    self.options.data
  }

  async fn resolve_version(&self, bundle_name: &str) -> crate::Result<BundleSourceVersion> {
    self
      .load_version(bundle_name)
      .await?
      .ok_or(crate::Error::BundleNotFound)
  }

  fn filepath_for(
    &self,
    bundle_name: &str,
    version: &BundleSourceVersion,
  ) -> crate::Result<PathBuf> {
    match version.kind {
      BundleSourceKind::Builtin => self.get_builtin_bundle_filepath(bundle_name, &version.version),
      BundleSourceKind::Remote => self.get_remote_bundle_filepath(bundle_name, &version.version),
    }
  }

  /// Whether bundles of this kind are verified against their manifest metadata on load.
  #[cfg(feature = "integrity")]
  fn verifies_on_load(&self, kind: &BundleSourceKind) -> bool {
    match self.options.verify_on_load {
      VerifyOnLoad::None => false,
      VerifyOnLoad::Remote => *kind == BundleSourceKind::Remote,
      VerifyOnLoad::All => true,
    }
  }

  /// Reads and verifies a bundle file against the integrity/signature recorded for it in
  /// the manifest.
  ///
  /// Returns the raw bytes when verification ran — the caller parses the bundle from them
  /// rather than re-reading the file — and `None` when this source does not verify this
  /// kind of bundle, leaving the caller on its lazy read path.
  async fn verified_bytes(
    &self,
    filepath: &Path,
    bundle_name: &str,
    version: &BundleSourceVersion,
  ) -> crate::Result<Option<Vec<u8>>> {
    #[cfg(feature = "integrity")]
    {
      if !self.verifies_on_load(&version.kind) {
        return Ok(None);
      }
      let metadata = match version.kind {
        BundleSourceKind::Builtin => {
          self
            .load_builtin_metadata(bundle_name, &version.version)
            .await?
        }
        BundleSourceKind::Remote => {
          self
            .load_remote_metadata(bundle_name, &version.version)
            .await?
        }
      }
      .unwrap_or_default();

      let data = read_file(filepath).await?;
      self
        .options
        .verify
        .verify(
          metadata.integrity.as_deref(),
          metadata.signature.as_deref(),
          &data,
        )
        .await?;
      Ok(Some(data))
    }
    #[cfg(not(feature = "integrity"))]
    {
      let _ = (filepath, bundle_name, version);
      Ok(None)
    }
  }

  async fn read_bundle(
    &self,
    filepath: &Path,
    bundle_name: &str,
    version: &BundleSourceVersion,
  ) -> crate::Result<Bundle> {
    if let Some(data) = self.verified_bytes(filepath, bundle_name, version).await? {
      return Reader::<Bundle>::read(&mut BundleReader::new(Cursor::new(&data)));
    }
    let mut file = open_file(filepath).await?;
    AsyncReader::<Bundle>::read(&mut AsyncBundleReader::new(&mut file)).await
  }

  async fn read_descriptor(
    &self,
    filepath: &Path,
    bundle_name: &str,
    version: &BundleSourceVersion,
  ) -> crate::Result<BundleDescriptor> {
    if let Some(data) = self.verified_bytes(filepath, bundle_name, version).await? {
      return Reader::<BundleDescriptor>::read(&mut BundleReader::new(Cursor::new(&data)));
    }
    let mut file = open_file(filepath).await?;
    AsyncReader::<BundleDescriptor>::read(&mut AsyncBundleReader::new(&mut file)).await
  }

  pub fn get_builtin_bundle_filepath(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<PathBuf> {
    self.get_filepath(&self.builtin_dir, bundle_name, version)
  }

  pub fn get_remote_bundle_filepath(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<PathBuf> {
    self.get_filepath(&self.remote_dir, bundle_name, version)
  }

  pub async fn fetch_bundle(&self, bundle_name: &str) -> crate::Result<Bundle> {
    let version = self.resolve_version(bundle_name).await?;
    let filepath = self.filepath_for(bundle_name, &version)?;
    self.read_bundle(&filepath, bundle_name, &version).await
  }

  pub async fn fetch_builtin_bundle(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Bundle> {
    let version = BundleSourceVersion::builtin(version.to_string());
    let filepath = self.filepath_for(bundle_name, &version)?;
    self.read_bundle(&filepath, bundle_name, &version).await
  }

  pub async fn fetch_remote_bundle(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Bundle> {
    let version = BundleSourceVersion::remote(version.to_string());
    let filepath = self.filepath_for(bundle_name, &version)?;
    self.read_bundle(&filepath, bundle_name, &version).await
  }

  pub async fn fetch_descriptor(&self, bundle_name: &str) -> crate::Result<BundleDescriptor> {
    let version = self.resolve_version(bundle_name).await?;
    let filepath = self.filepath_for(bundle_name, &version)?;
    self.read_descriptor(&filepath, bundle_name, &version).await
  }

  pub async fn load_builtin_metadata(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Option<BundleManifestMetadata>> {
    self
      .builtin_manifest
      .load_metadata(bundle_name, version)
      .await
  }

  pub async fn load_remote_metadata(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Option<BundleManifestMetadata>> {
    self
      .remote_manifest
      .load_metadata(bundle_name, version)
      .await
  }

  pub async fn load_descriptor(&self, bundle_name: &str) -> crate::Result<Arc<LoadedDescriptor>> {
    let version = self.resolve_version(bundle_name).await?;
    let filepath = self.filepath_for(bundle_name, &version)?;
    let cell = match self.descriptors.entry(bundle_name.to_string()) {
      dashmap::Entry::Occupied(mut occupied) => {
        let (cached_path, cell) = occupied.get();
        if cached_path == &filepath {
          cell.clone()
        } else {
          // The active version changed since this entry was cached: drop the
          // stale cell so the descriptor is reloaded from the new filepath.
          let cell = Arc::new(OnceCell::new());
          occupied.insert((filepath.clone(), cell.clone()));
          cell
        }
      }
      dashmap::Entry::Vacant(vacant) => {
        let cell = Arc::new(OnceCell::new());
        vacant.insert((filepath.clone(), cell.clone()));
        cell
      }
    };
    // Verification (when enabled) happens inside the cell, so a bundle is hashed once per
    // version rather than once per request, and concurrent first loads single-flight into
    // one verification.
    let descriptor = cell
      .get_or_try_init(|| async {
        let d = self
          .read_descriptor(&filepath, bundle_name, &version)
          .await?;
        Ok::<Arc<BundleDescriptor>, crate::Error>(Arc::new(d))
      })
      .await?
      .clone();
    Ok(Arc::new(LoadedDescriptor {
      descriptor,
      filepath,
      data_options: self.options.data,
    }))
  }

  pub fn unload_descriptor(&self, bundle_name: &str) -> bool {
    self.descriptors.remove(bundle_name).is_some()
  }

  pub async fn write_remote_bundle(
    &self,
    bundle_name: &str,
    version: &str,
    bundle: &Bundle,
    metadata: BundleManifestMetadata,
  ) -> crate::Result<()> {
    let mut data = vec![];
    Writer::<Bundle>::write(
      &mut crate::BundleWriter::new(Cursor::new(&mut data)),
      bundle,
    )?;
    self
      .write_remote_bundle_data(bundle_name, version, &data, metadata)
      .await
  }

  /// Writes the raw bytes of a `.wvb` file to the remote directory and records it in the
  /// manifest.
  ///
  /// Prefer this over [`BundleSource::write_remote_bundle`] when the bytes are already at
  /// hand (e.g. straight from a download): the integrity string in `metadata` covers those
  /// exact bytes, and storing them verbatim — rather than re-serializing a parsed
  /// [`Bundle`] — is what lets the file be verified again on every later load.
  pub async fn write_remote_bundle_data(
    &self,
    bundle_name: &str,
    version: &str,
    data: &[u8],
    metadata: BundleManifestMetadata,
  ) -> crate::Result<()> {
    let filepath = self.get_remote_bundle_filepath(bundle_name, version)?;
    if let Some(parent) = filepath.parent() {
      let _ = tokio::fs::create_dir_all(parent).await;
    }

    // Write to a temp file then atomically rename into place.
    let seq = WRITE_SEQ.fetch_add(1, Ordering::Relaxed);

    let mut tmp = filepath.clone().into_os_string();
    tmp.push(format!(".{seq}.tmp"));

    let tmp = PathBuf::from(tmp);
    let mut file = File::create(&tmp).await?;

    file.write_all(data).await?;
    file.flush().await?;
    drop(file); // close the temp handle before rename (required on Windows)

    if let Err(e) = tokio::fs::rename(&tmp, &filepath).await {
      let _ = tokio::fs::remove_file(&tmp).await;
      return Err(e.into());
    }

    self
      .remote_manifest
      .insert_entry(bundle_name, version, metadata)
      .await?;
    self.remote_manifest.save().await?;
    Ok(())
  }

  /// Removes a single staged remote bundle: drops its manifest entry and deletes its
  /// file from disk. Returns whether the entry existed.
  pub async fn remove_remote_bundle(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<bool> {
    let removed = self
      .remote_manifest
      .remove_entry(bundle_name, version)
      .await?;
    if removed {
      let filepath = self.get_remote_bundle_filepath(bundle_name, version)?;
      let _ = tokio::fs::remove_file(&filepath).await;
      self.remote_manifest.save().await?;
    }
    Ok(removed)
  }

  pub async fn remote_retained_versions(&self, bundle_name: &str) -> crate::Result<Vec<String>> {
    self.remote_manifest.retained_versions(bundle_name).await
  }

  /// Removes every staged remote version except the retained set ({current, previous}).
  pub async fn prune_remote_bundles(&self, bundle_name: &str) -> crate::Result<Vec<String>> {
    let retained = self.remote_retained_versions(bundle_name).await?;
    let all = self.remote_manifest.list_versions(bundle_name).await?;
    let mut removed = vec![];
    for version in all {
      if retained.contains(&version) {
        continue;
      }
      if self
        .remove_remote_bundle(bundle_name, &version)
        .await
        .unwrap_or(false)
      {
        removed.push(version);
      }
    }
    Ok(removed)
  }

  fn get_filepath(
    &self,
    base_dir: &Path,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<PathBuf> {
    let filename = format!("{bundle_name}_{version}.{EXTENSION}");
    let filepath = base_dir.join(bundle_name).join(filename);
    if !is_valid_path_component(bundle_name) || !is_valid_path_component(version) {
      return Err(crate::Error::invalid_filepath(filepath.to_string_lossy()));
    }
    Ok(filepath)
  }
}

/// Returns whether `value` is safe to use verbatim as a single filesystem path component on
/// Windows, macOS, and Linux.
fn is_valid_path_component(value: &str) -> bool {
  !value.is_empty()
    && value != "."
    && value != ".."
    && value
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    // Windows strips a trailing `.`, which would collapse distinct names (e.g. "app." and "app")
    // onto the same file. Reject it so resolved filepaths stay unambiguous across platforms.
    && !value.ends_with('.')
    && !is_windows_reserved_name(value)
}

const WINDOWS_RESERVED_NAMES: &[&str] = &[
  "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
  "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn is_windows_reserved_name(value: &str) -> bool {
  let base = value.split('.').next().unwrap_or(value);
  WINDOWS_RESERVED_NAMES
    .iter()
    .any(|reserved| reserved.eq_ignore_ascii_case(base))
}

fn map_read_error(e: std::io::Error) -> crate::Error {
  if e.kind() == std::io::ErrorKind::NotFound {
    return crate::Error::BundleNotFound;
  }
  crate::Error::from(e)
}

async fn open_file(filepath: &Path) -> crate::Result<File> {
  File::open(filepath).await.map_err(map_read_error)
}

#[cfg(feature = "integrity")]
async fn read_file(filepath: &Path) -> crate::Result<Vec<u8>> {
  tokio::fs::read(filepath).await.map_err(map_read_error)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::testing::Fixtures;

  #[test]
  fn valid_path_component() {
    for ok in [
      "app",
      "my-app",
      "my_app",
      "App2",
      "1.0.0",
      "1.2.3-beta.4",
      "a.b.c",
      // Merely starting with a reserved word, or COM/LPT without a digit, is fine.
      "console",
      "com",
      "com10",
    ] {
      assert!(is_valid_path_component(ok), "{ok:?} should be valid");
    }
    for bad in [
      "",
      ".",
      "..",
      "a/b",
      "a\\b",
      "../etc",
      "a b",
      "안녕",
      "a\nb",
      "a\0b",
      // Windows reserved device names (case-insensitive, with or without an extension).
      "con",
      "CON",
      "NuL",
      "com1",
      "LPT9",
      "aux",
      "prn",
      "nul.txt",
      "con.foo.bar",
      // Trailing dot — Windows strips it, collapsing distinct names onto the same file.
      "app.",
      "1.0.0.",
    ] {
      assert!(!is_valid_path_component(bad), "{bad:?} should be invalid");
    }
  }

  #[test]
  fn invalid_filepath() {
    let source = BundleSource::builder()
      .builtin_dir("/tmp/builtin")
      .remote_dir("/tmp/remote")
      .build();

    // Valid name + version resolve to a path.
    assert!(source.get_remote_bundle_filepath("app", "1.0.0").is_ok());
    assert!(
      source
        .get_builtin_bundle_filepath("my-app", "1.2.3-beta.4")
        .is_ok()
    );

    // An unsafe bundle name cannot be turned into a filepath.
    for name in ["", "..", "a/b", "../etc", "a b"] {
      assert!(
        matches!(
          source.get_remote_bundle_filepath(name, "1.0.0"),
          Err(crate::Error::InvalidFilepath(_))
        ),
        "name {name:?} should be rejected"
      );
    }

    // An unsafe version is rejected too.
    for version in ["", "..", "1/0", "1 0"] {
      assert!(
        matches!(
          source.get_remote_bundle_filepath("app", version),
          Err(crate::Error::InvalidFilepath(_))
        ),
        "version {version:?} should be rejected"
      );
    }
  }

  #[tokio::test]
  async fn invalid_filepath_when_write_remote_bundle() {
    let fixture = Fixtures::bundles();
    let source = BundleSource::builder()
      .remote_dir(fixture.get_path("remote"))
      .build();
    let bundle = crate::BundleBuilder::new().build().unwrap();
    let err = source
      .write_remote_bundle("../evil", "1.0.0", &bundle, Default::default())
      .await
      .unwrap_err();
    assert!(matches!(err, crate::Error::InvalidFilepath(_)));
  }

  #[tokio::test]
  async fn fetch() {
    let fixture = Fixtures::bundles();
    let source = BundleSource::builder()
      .builtin_dir(fixture.get_path("builtin"))
      .remote_dir(fixture.get_path("remote"))
      .build();
    let bundle = source.fetch_bundle("app").await.unwrap();
    bundle.get_data("/index.html").unwrap().unwrap();
  }

  #[tokio::test]
  async fn fetch_descriptor() {
    let fixture = Fixtures::bundles();
    let source = BundleSource::builder()
      .builtin_dir(fixture.get_path("builtin"))
      .remote_dir(fixture.get_path("remote"))
      .build();
    let descriptor = source.fetch_descriptor("app").await.unwrap();
    assert!(descriptor.index().contains_path("/index.html"));
  }

  #[tokio::test]
  async fn fetch_many_times() {
    let fixture = Fixtures::bundles();
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(fixture.get_path("builtin"))
        .remote_dir(fixture.get_path("remote"))
        .build(),
    );
    let mut handles = Vec::new();
    for _i in 0..10 {
      let s = source.clone();
      let handle = tokio::spawn(async move {
        let bundle = s.fetch_bundle("app").await.unwrap();
        bundle.get_data("/index.html").unwrap().unwrap();
      });
      handles.push(handle);
    }
    for h in handles {
      h.await.unwrap();
    }
  }

  #[tokio::test]
  async fn source_version_not_found() {
    let fixture = Fixtures::bundles();
    let source = BundleSource::builder()
      .builtin_dir(fixture.get_path("builtin"))
      .remote_dir(fixture.get_path("remote"))
      .build();
    let bundle = source.fetch_bundle("not-found").await;
    assert!(matches!(bundle.unwrap_err(), crate::Error::BundleNotFound));
  }

  #[tokio::test]
  async fn load_many_at_once() {
    let fixture = Fixtures::bundles();
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(fixture.get_path("builtin"))
        .remote_dir(fixture.get_path("remote"))
        .build(),
    );
    let mut handles = Vec::new();
    for _i in 0..10 {
      let s = source.clone();
      let handle = tokio::spawn(async move {
        let _ = s.load_descriptor("app.wvb").await;
      });
      handles.push(handle);
    }
    for h in handles {
      h.await.unwrap();
    }
  }

  #[tokio::test]
  async fn load_and_unload_sequential() {
    let fixture = Fixtures::bundles();
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(fixture.get_path("builtin"))
        .remote_dir(fixture.get_path("remote"))
        .build(),
    );
    let m1 = source.load_descriptor("app").await.unwrap();
    assert!(
      source.unload_descriptor("app"),
      "unload should remove existing entry"
    );
    let m2 = source.load_descriptor("app").await.unwrap();
    assert!(
      !Arc::ptr_eq(m1.descriptor(), m2.descriptor()),
      "after unload, reloading should produce a new descriptor"
    );

    assert!(source.unload_descriptor("app"));
    let m3 = source.load_descriptor("app").await.unwrap();
    assert!(!Arc::ptr_eq(m2.descriptor(), m3.descriptor()));

    assert!(source.unload_descriptor("app"));
    let m4 = source.load_descriptor("app").await.unwrap();
    assert!(!Arc::ptr_eq(m3.descriptor(), m4.descriptor()));
  }

  #[tokio::test]
  async fn load_and_unload_concurrently() {
    use std::sync::Arc;
    use tokio::sync::Barrier;
    use tokio::task::JoinSet;

    let fixture = Fixtures::bundles();
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(fixture.get_path("builtin"))
        .remote_dir(fixture.get_path("remote"))
        .build(),
    );

    // 1) initial loads. test single flight
    let n = 5usize;
    let mut set = JoinSet::new();
    for _i in 0..n {
      let s = source.clone();
      set.spawn(async move { s.load_descriptor("app").await });
    }
    let mut initials = Vec::with_capacity(n);
    while let Some(res) = set.join_next().await {
      let v = res.unwrap().unwrap();
      initials.push(v);
    }
    for m in &initials[1..] {
      assert!(Arc::ptr_eq(initials[0].descriptor(), m.descriptor()));
    }

    // 2) before/after barriers
    let barrier_before_unload = Arc::new(Barrier::new(n + 1));
    let barrier_after_unload = Arc::new(Barrier::new(n + 1));

    let mut before_set = JoinSet::new();
    for _i in 0..n {
      let s = source.clone();
      let before = barrier_before_unload.clone();
      before_set.spawn(async move {
        before.wait().await;
        s.load_descriptor("app").await
      });
    }
    let mut after_set = JoinSet::new();
    for _i in 0..n {
      let s = source.clone();
      let after = barrier_after_unload.clone();
      after_set.spawn(async move {
        after.wait().await;
        s.load_descriptor("app").await
      });
    }

    barrier_before_unload.wait().await;
    assert!(source.unload_descriptor("app"));
    barrier_after_unload.wait().await;

    let mut before_jobs = Vec::with_capacity(n);
    while let Some(res) = before_set.join_next().await {
      let v = res.unwrap().unwrap();
      before_jobs.push(v);
    }
    let mut after_jobs = Vec::with_capacity(n);
    while let Some(res) = after_set.join_next().await {
      let v = res.unwrap().unwrap();
      after_jobs.push(v);
    }
    // before jobs should be same with initial loads
    for m in &before_jobs {
      assert!(Arc::ptr_eq(initials[0].descriptor(), m.descriptor()));
    }
    // after jobs should be not same with initial loads
    for m in &after_jobs {
      assert!(!Arc::ptr_eq(initials[0].descriptor(), m.descriptor()));
    }
    for m in &before_jobs[1..] {
      assert!(Arc::ptr_eq(before_jobs[0].descriptor(), m.descriptor()));
    }
    for m in &after_jobs[1..] {
      assert!(Arc::ptr_eq(after_jobs[0].descriptor(), m.descriptor()));
    }
  }
}
