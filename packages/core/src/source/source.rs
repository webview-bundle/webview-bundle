#[cfg(feature = "integrity")]
use crate::integrity::IntegrityPolicy;
#[cfg(feature = "signature")]
use crate::signature::SignatureVerify;
use crate::source::types::BundleSourceKind;
use crate::source::{
  BundleManifest, BundleManifestEntryItem, BundleManifestEntryItemStatus,
  BundleManifestVersionData, BundleSourceOptions, BundleSourceVersion, ReadOnly, ReadWrite,
};
use crate::util;
use crate::{
  AsyncBundleReader, AsyncReader, Bundle, BundleDescriptor, BundleReader, DataReadOptions,
  EXTENSION, MANIFEST_FILENAME, Reader, Writer,
};
use dashmap::DashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::File;
use tokio::sync::OnceCell;

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

  #[must_use]
  pub fn options(mut self, options: BundleSourceOptions) -> Self {
    self.options = options;
    self
  }

  pub fn build(self) -> BundleSource {
    let builtin_dir = self.builtin_dir;
    let builtin_manifest_filepath = self
      .builtin_manifest_filepath
      .map(|x| util::fs::normalize_path(&builtin_dir, &x))
      .unwrap_or(builtin_dir.join(MANIFEST_FILENAME));
    let remote_dir = self.remote_dir;
    let remote_manifest_filepath = self
      .remote_manifest_filepath
      .map(|x| util::fs::normalize_path(&remote_dir, &x))
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
  pub source: BundleSourceKind,
  #[cfg_attr(feature = "_serde", serde(flatten))]
  pub item: BundleManifestEntryItem,
}

impl ListBundleItem {
  pub(crate) fn from(source: BundleSourceKind, item: BundleManifestEntryItem) -> Self {
    Self { source, item }
  }
}

/// A lazily-initialized descriptor cell, shared so concurrent loads single-flight.
type DescriptorCell = Arc<OnceCell<Arc<BundleDescriptor>>>;

#[derive(Debug)]
pub struct BundleSource {
  builtin_dir: PathBuf,
  builtin_manifest: BundleManifest<ReadOnly>,
  remote_dir: PathBuf,
  remote_manifest: BundleManifest<ReadWrite>,
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
  data_read_options: DataReadOptions,
}

impl LoadedDescriptor {
  pub async fn reader(&self) -> crate::Result<File> {
    open_file(&self.filepath).await
  }

  pub fn descriptor(&self) -> &Arc<BundleDescriptor> {
    &self.descriptor
  }

  pub fn data_read_options(&self) -> &DataReadOptions {
    &self.data_read_options
  }

  /// Reads the data for `path`, lazily from the bundle file this descriptor was loaded
  /// from, applying the source's [`DataReadOptions`].
  ///
  /// Returns `None` if the path doesn't exist in the bundle.
  pub async fn get_data(&self, path: &str) -> crate::Result<Option<Vec<u8>>> {
    self
      .get_data_with_options(path, self.data_read_options)
      .await
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

impl BundleSource {
  pub fn builder() -> BundleSourceBuilder {
    BundleSourceBuilder::new()
  }

  pub fn options(&self) -> &BundleSourceOptions {
    &self.options
  }

  pub async fn list_bundles(&self) -> crate::Result<Vec<ListBundleItem>> {
    let (builtin_items, remote_items) =
      tokio::try_join!(self.list_builtin_bundles(), self.list_remote_bundles(),)?;
    Ok([builtin_items, remote_items].concat())
  }

  pub async fn list_builtin_bundles(&self) -> crate::Result<Vec<ListBundleItem>> {
    let entries = self.builtin_manifest.list_entries().await?;
    let items = entries
      .into_iter()
      .map(|item| ListBundleItem::from(BundleSourceKind::Builtin, item))
      .collect::<Vec<_>>();
    Ok(items)
  }

  pub async fn list_remote_bundles(&self) -> crate::Result<Vec<ListBundleItem>> {
    let entries = self.remote_manifest.list_entries().await?;
    let items = entries
      .into_iter()
      .map(|item| ListBundleItem::from(BundleSourceKind::Builtin, item))
      .collect::<Vec<_>>();
    Ok(items)
  }

  pub async fn get_version(&self, bundle_name: &str) -> crate::Result<Option<BundleSourceVersion>> {
    match self
      .remote_manifest
      .get_current_version(bundle_name)
      .await?
    {
      Some(ver) => Ok(Some(BundleSourceVersion::remote(ver))),
      None => {
        // fallback to builtin version
        let builtin_version = self
          .builtin_manifest
          .get_current_version(bundle_name)
          .await?
          .map(BundleSourceVersion::builtin);
        Ok(builtin_version)
      }
    }
  }

  pub async fn update_remote_version(&self, bundle_name: &str, version: &str) -> crate::Result<()> {
    self
      .update_remote_versions(&[(bundle_name.to_owned(), version.to_owned())])
      .await
  }

  pub async fn update_remote_versions(&self, items: &[(String, String)]) -> crate::Result<()> {
    if items.is_empty() {
      return Ok(());
    }
    self.remote_manifest.set_current_versions(items).await?;
    self.remote_manifest.save().await?;
    Ok(())
  }

  pub async fn stage_remote_bundles(
    &self,
    items: &[(String, String, BundleManifestVersionData)],
  ) -> crate::Result<()> {
    if items.is_empty() {
      return Ok(());
    }
    self.remote_manifest.insert_staged_entries(items).await?;
    self.remote_manifest.save().await?;
    Ok(())
  }

  pub async fn get_remote_staged_version(
    &self,
    bundle_name: &str,
  ) -> crate::Result<Option<String>> {
    self.remote_manifest.get_staged_version(bundle_name).await
  }

  pub async fn resolve_filepath(&self, bundle_name: &str) -> crate::Result<PathBuf> {
    let ver = self.resolve_version(bundle_name).await?;
    self.filepath_for(bundle_name, &ver)
  }

  async fn resolve_version(&self, bundle_name: &str) -> crate::Result<BundleSourceVersion> {
    self
      .get_version(bundle_name)
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

  /// Whether the integrity of bundles of this kind is checked on load.
  #[cfg(feature = "integrity")]
  fn checks_integrity_on_load(&self, kind: &BundleSourceKind) -> bool {
    self.options.integrity.policy != IntegrityPolicy::Off
      && self.options.integrity.check_mode.should_verify(kind)
  }

  /// The signature verifier applied to bundles of this kind on load, if any.
  #[cfg(feature = "signature")]
  fn signature_verifier_on_load(&self, kind: &BundleSourceKind) -> Option<&SignatureVerify> {
    match self.options.signature.verify_mode.should_verify(kind) {
      true => self.options.signature.verify.as_ref(),
      false => None,
    }
  }

  async fn verified_bytes(
    &self,
    filepath: &Path,
    bundle_name: &str,
    version: &BundleSourceVersion,
  ) -> crate::Result<Option<Vec<u8>>> {
    #[cfg(feature = "integrity")]
    {
      let check_integrity = self.checks_integrity_on_load(&version.kind);
      #[cfg(feature = "signature")]
      let signature_verifier = self.signature_verifier_on_load(&version.kind);
      #[cfg(feature = "signature")]
      let verify_signature = signature_verifier.is_some();
      #[cfg(not(feature = "signature"))]
      let verify_signature = false;

      if !check_integrity && !verify_signature {
        return Ok(None);
      }

      let metadata = match version.kind {
        BundleSourceKind::Builtin => {
          self
            .get_builtin_metadata(bundle_name, &version.version)
            .await?
        }
        BundleSourceKind::Remote => {
          self
            .get_remote_metadata(bundle_name, &version.version)
            .await?
        }
      }
      .unwrap_or_default();

      // The signature covers the integrity string, not the file, so only the integrity
      // check needs the bytes.
      let data = match check_integrity && metadata.integrity.is_some() {
        true => Some(read_file(filepath).await?),
        false => None,
      };
      if check_integrity {
        crate::integrity::verify_integrity(
          &self.options.integrity.policy,
          &self.options.integrity.check,
          metadata.integrity.as_deref(),
          data.as_deref().unwrap_or_default(),
        )
        .await?;
      }

      #[cfg(feature = "signature")]
      if let Some(verify) = signature_verifier {
        crate::signature::verify_signature(
          verify,
          metadata.integrity.as_deref(),
          metadata.signature.as_deref(),
        )
        .await?;
      }

      Ok(data)
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
      return Reader::<Bundle>::read(&mut BundleReader::new_with_options(
        Cursor::new(&data),
        self.options.header_read,
        self.options.index_read,
      ));
    }
    let mut file = open_file(filepath).await?;
    AsyncReader::<Bundle>::read(&mut AsyncBundleReader::new_with_options(
      &mut file,
      self.options.header_read,
      self.options.index_read,
    ))
    .await
  }

  async fn read_descriptor(
    &self,
    filepath: &Path,
    bundle_name: &str,
    version: &BundleSourceVersion,
  ) -> crate::Result<BundleDescriptor> {
    if let Some(data) = self.verified_bytes(filepath, bundle_name, version).await? {
      return Reader::<BundleDescriptor>::read(&mut BundleReader::new_with_options(
        Cursor::new(&data),
        self.options.header_read,
        self.options.index_read,
      ));
    }
    let mut file = open_file(filepath).await?;
    AsyncReader::<BundleDescriptor>::read(&mut AsyncBundleReader::new_with_options(
      &mut file,
      self.options.header_read,
      self.options.index_read,
    ))
    .await
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

  pub async fn get_builtin_metadata(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Option<BundleManifestVersionData>> {
    self
      .builtin_manifest
      .get_entry_data(bundle_name, version)
      .await
  }

  pub async fn get_remote_metadata(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Option<BundleManifestVersionData>> {
    self
      .remote_manifest
      .get_entry_data(bundle_name, version)
      .await
  }

  pub async fn load(&self, bundle_name: &str) -> crate::Result<Arc<LoadedDescriptor>> {
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
      data_read_options: self.options.data_read,
    }))
  }

  pub fn unload(&self, bundle_name: &str) -> bool {
    self.descriptors.remove(bundle_name).is_some()
  }

  /// Removes a single staged remote bundle: drops its manifest entry and deletes its
  /// file from disk. Returns whether the entry existed.
  pub async fn remove_remote_bundle(
    &self,
    bundle_name: &str,
    version: &str,
    force: Option<bool>,
  ) -> crate::Result<bool> {
    let removed = self
      .remote_manifest
      .remove_entry(bundle_name, version, force)
      .await?;
    if removed {
      if let Ok(filepath) = self.get_remote_bundle_filepath(bundle_name, version) {
        let _ = tokio::fs::remove_file(&filepath).await;
      }
      self.remote_manifest.save().await?;
    }
    Ok(removed)
  }

  pub async fn remove_remote_bundles(
    &self,
    items: Vec<BundleManifestEntryItem>,
    force: Option<bool>,
  ) -> crate::Result<Vec<BundleManifestEntryItem>> {
    let removed = self.remote_manifest.remove_entries(items, force).await?;
    if !removed.is_empty() {
      self.remote_manifest.save().await?;
    }

    Ok(removed)
  }

  /// Remove orphan remote bundles which is not using so can free disk space.
  pub async fn prune_remote_bundles(
    &self,
    bundle_name: &str,
  ) -> crate::Result<Vec<BundleManifestEntryItem>> {
    let bundle_names = [bundle_name.to_owned()];
    self.prune_remote_bundles_many(&bundle_names).await
  }

  /// Same as [`BundleSource::prune_remote_bundles`] for several bundles, using a single
  /// manifest write.
  pub async fn prune_remote_bundles_many(
    &self,
    bundle_names: &[String],
  ) -> crate::Result<Vec<BundleManifestEntryItem>> {
    let items = self
      .remote_manifest
      .list_entries()
      .await?
      .into_iter()
      .filter(|x| {
        x.status == BundleManifestEntryItemStatus::Orphan && bundle_names.contains(&x.name)
      })
      .collect::<Vec<_>>();
    let removed = self.remove_remote_bundles(items, None).await?;
    Ok(removed)
  }

  fn get_filepath(
    &self,
    base_dir: &Path,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<PathBuf> {
    let filename = format!("{version}.{EXTENSION}");
    let filepath = base_dir.join(bundle_name).join(filename);
    if !is_valid_path_component(bundle_name) || !is_valid_path_component(version) {
      return Err(crate::Error::invalid_filepath(filepath.to_string_lossy()));
    }
    Ok(filepath)
  }
}

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
  if e.kind() == std::io::ErrorKind::NotFound
    || (cfg!(windows) && e.kind() == std::io::ErrorKind::PermissionDenied)
  {
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
  fn map_read_error_treats_missing_file_as_bundle_not_found() {
    let e = std::io::Error::from(std::io::ErrorKind::NotFound);
    assert!(matches!(map_read_error(e), crate::Error::BundleNotFound));
  }

  #[cfg(windows)]
  #[test]
  fn map_read_error_treats_delete_pending_as_bundle_not_found() {
    let e = std::io::Error::from_raw_os_error(5);
    assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(matches!(map_read_error(e), crate::Error::BundleNotFound));
  }

  #[cfg(not(windows))]
  #[test]
  fn map_read_error_keeps_permission_denied_as_io() {
    let e = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
    assert!(matches!(map_read_error(e), crate::Error::Io(_)));
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
        let _ = s.load("app.wvb").await;
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
    let m1 = source.load("app").await.unwrap();
    assert!(source.unload("app"), "unload should remove existing entry");
    let m2 = source.load("app").await.unwrap();
    assert!(
      !Arc::ptr_eq(m1.descriptor(), m2.descriptor()),
      "after unload, reloading should produce a new descriptor"
    );

    assert!(source.unload("app"));
    let m3 = source.load("app").await.unwrap();
    assert!(!Arc::ptr_eq(m2.descriptor(), m3.descriptor()));

    assert!(source.unload("app"));
    let m4 = source.load("app").await.unwrap();
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
      set.spawn(async move { s.load("app").await });
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
        s.load("app").await
      });
    }
    let mut after_set = JoinSet::new();
    for _i in 0..n {
      let s = source.clone();
      let after = barrier_after_unload.clone();
      after_set.spawn(async move {
        after.wait().await;
        s.load("app").await
      });
    }

    barrier_before_unload.wait().await;
    assert!(source.unload("app"));
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
