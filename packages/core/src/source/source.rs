#[cfg(feature = "integrity")]
use crate::integrity::IntegrityPolicy;
use crate::source::types::SourceKind;
use crate::source::{
  BundleSourceVersion, Manifest, ManifestPruneResult, ManifestRemoveData, ManifestRemoveResult,
  ManifestRemoveResultKind, ManifestSetCurrentVersionResult, ManifestStageData,
  ManifestStageResult, ManifestVersionData, ReadOnly, ReadWrite, SourceListItem, SourceOptions,
};
use crate::util;
use crate::{
  AsyncBundleReader, AsyncReader, Bundle, BundleDescriptor, BundleReader, DataReadOptions,
  EXTENSION, MANIFEST_FILENAME, Reader,
};
use dashmap::DashMap;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::File;
use tokio::sync::{OnceCell, RwLock};

/// Builder for creating a `Source`.
///
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "source")]
/// # {
/// use wvb::source::Source;
///
/// let source = Source::builder()
///     .builtin_dir("./builtin")
///     .remote_dir("./remote")
///     .build();
/// # }
/// ```
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct SourceBuilder {
  builtin_dir: PathBuf,
  builtin_manifest_filepath: Option<PathBuf>,
  remote_dir: PathBuf,
  remote_manifest_filepath: Option<PathBuf>,
  options: SourceOptions,
}

impl SourceBuilder {
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
  pub fn options(mut self, options: SourceOptions) -> Self {
    self.options = options;
    self
  }

  pub fn build(self) -> Source {
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
    Source {
      builtin_dir,
      builtin_manifest: Manifest::new(&builtin_manifest_filepath, ReadOnly),
      remote_dir,
      remote_manifest: RwLock::new(Manifest::new(&remote_manifest_filepath, ReadWrite)),
      descriptors: DashMap::default(),
      options: self.options,
    }
  }
}

/// A lazily-initialized descriptor cell, shared so concurrent loads single-flight.
type DescriptorCell = Arc<OnceCell<Arc<BundleDescriptor>>>;

#[derive(Debug)]
pub struct Source {
  builtin_dir: PathBuf,
  pub(crate) builtin_manifest: Manifest<ReadOnly>,
  remote_dir: PathBuf,
  pub(crate) remote_manifest: RwLock<Manifest<ReadWrite>>,
  descriptors: DashMap<String, (PathBuf, DescriptorCell)>,
  options: SourceOptions,
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

impl Source {
  pub fn builder() -> SourceBuilder {
    SourceBuilder::new()
  }

  pub fn options(&self) -> &SourceOptions {
    &self.options
  }

  pub fn builtin_dir(&self) -> &Path {
    &self.builtin_dir
  }

  pub fn builtin_manifest(&self) -> &Manifest<ReadOnly> {
    &self.builtin_manifest
  }

  pub fn remote_dir(&self) -> &Path {
    &self.remote_dir
  }

  pub fn remote_manifest(&self) -> &RwLock<Manifest<ReadWrite>> {
    &self.remote_manifest
  }

  pub async fn list_bundles(&self) -> crate::Result<Vec<SourceListItem>> {
    let (builtin_items, remote_items) =
      tokio::try_join!(self.list_builtin_bundles(), self.list_remote_bundles())?;
    Ok([builtin_items, remote_items].concat())
  }

  pub async fn list_builtin_bundles(&self) -> crate::Result<Vec<SourceListItem>> {
    let items = self
      .builtin_manifest
      .list_items()
      .await?
      .into_iter()
      .map(|item| SourceListItem::from(SourceKind::Builtin, item))
      .collect::<Vec<_>>();
    Ok(items)
  }

  pub async fn list_remote_bundles(&self) -> crate::Result<Vec<SourceListItem>> {
    let remote_manifest = self.remote_manifest.read().await;
    let items = remote_manifest
      .list_items()
      .await?
      .into_iter()
      .map(|item| SourceListItem::from(SourceKind::Remote, item))
      .collect::<Vec<_>>();
    Ok(items)
  }

  pub async fn get_version(&self, bundle_name: &str) -> crate::Result<Option<BundleSourceVersion>> {
    let remote_manifest = self.remote_manifest.read().await;
    match remote_manifest.get_current_version(bundle_name).await? {
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

  pub async fn get_remote_staged_version(
    &self,
    bundle_name: &str,
  ) -> crate::Result<Option<String>> {
    let remote_manifest = self.remote_manifest.read().await;
    let version = remote_manifest.get_staged_version(bundle_name).await?;
    Ok(version)
  }

  pub async fn get_remote_previous_version(
    &self,
    bundle_name: &str,
  ) -> crate::Result<Option<String>> {
    let remote_manifest = self.remote_manifest.read().await;
    let version = remote_manifest.get_previous_version(bundle_name).await?;
    Ok(version)
  }

  pub async fn update_remote_version(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<ManifestSetCurrentVersionResult> {
    let mut remote_manifest = self.remote_manifest.write().await;
    let result = remote_manifest
      .set_current_version(bundle_name, version)
      .await?;
    Ok(result)
  }

  pub async fn update_remote_versions(
    &self,
    items: impl Into<HashMap<String, String>>,
  ) -> crate::Result<Vec<ManifestSetCurrentVersionResult>> {
    let mut remote_manifest = self.remote_manifest.write().await;
    let results = remote_manifest.set_current_version_many(items).await?;
    Ok(results)
  }

  pub async fn stage_remote_bundle(
    &self,
    bundle_name: &str,
    data: ManifestStageData,
  ) -> crate::Result<ManifestStageResult> {
    let mut remote_manifest = self.remote_manifest.write().await;
    let result = remote_manifest.stage(bundle_name, data).await?;
    Ok(result)
  }

  pub async fn stage_remote_bundles(
    &self,
    items: impl Into<HashMap<String, ManifestStageData>>,
  ) -> crate::Result<Vec<ManifestStageResult>> {
    let mut remote_manifest = self.remote_manifest.write().await;
    let results = remote_manifest.stage_many(items).await?;
    Ok(results)
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
    match version.source {
      SourceKind::Builtin => self.get_builtin_bundle_filepath(bundle_name, &version.version),
      SourceKind::Remote => self.get_remote_bundle_filepath(bundle_name, &version.version),
    }
  }

  #[cfg(feature = "integrity")]
  fn checks_integrity_on_load(&self, kind: &SourceKind) -> bool {
    self.options.integrity.policy != IntegrityPolicy::Off
      && self.options.integrity.check_mode.should_verify(kind)
  }

  async fn verified_bytes(
    &self,
    filepath: &Path,
    bundle_name: &str,
    version: &BundleSourceVersion,
  ) -> crate::Result<Option<Vec<u8>>> {
    #[cfg(feature = "integrity")]
    {
      let check_integrity = self.checks_integrity_on_load(&version.source);
      if !check_integrity {
        return Ok(None);
      }

      let data = match version.source {
        SourceKind::Builtin => {
          self
            .get_builtin_version_data(bundle_name, &version.version)
            .await?
        }
        SourceKind::Remote => {
          self
            .get_remote_version_data(bundle_name, &version.version)
            .await?
        }
      }
      .unwrap_or_default();

      // The signature covers the integrity string, not the file, so only the integrity
      // check needs the bytes.
      let file_data = match check_integrity && data.integrity.is_some() {
        true => Some(read_file(filepath).await?),
        false => None,
      };
      if check_integrity {
        crate::integrity::verify_integrity(
          &self.options.integrity.policy,
          data.integrity.as_deref(),
          file_data.as_deref().unwrap_or_default(),
        )?;
      }
      Ok(file_data)
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

  pub async fn get_builtin_version_data(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Option<ManifestVersionData>> {
    self
      .builtin_manifest
      .get_version_data(bundle_name, version)
      .await
  }

  pub async fn get_remote_version_data(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Option<ManifestVersionData>> {
    let remote_manifest = self.remote_manifest.read().await;
    let data = remote_manifest
      .get_version_data(bundle_name, version)
      .await?;
    Ok(data)
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

  fn unload_filepath(&self, bundle_name: &str, filepath: &Path) -> bool {
    self
      .descriptors
      .remove_if(bundle_name, |_, (cached, _)| cached == filepath)
      .is_some()
  }

  pub async fn remove_remote_bundle(
    &self,
    bundle_name: &str,
    version: &str,
    force: Option<bool>,
  ) -> crate::Result<ManifestRemoveResult> {
    // The file is unreferenced once the manifest no longer lists it, so deleting it does not
    // need the lock — and holding it there would stall every read for the whole deletion.
    let (result, filepath) = {
      let mut remote_manifest = self.remote_manifest.write().await;
      let result = remote_manifest.remove(bundle_name, version, force).await?;

      let mut filepath = None;
      if result.kind == ManifestRemoveResultKind::Removed {
        if let Ok(path) = self.get_remote_bundle_filepath(&result.name, &result.version) {
          self.unload_filepath(&result.name, &path);
          filepath = Some(path);
        }
      }
      (result, filepath)
    };

    if let Some(filepath) = filepath {
      remove_files_by_chunk(vec![filepath], Some(1)).await;
    }

    Ok(result)
  }

  pub async fn remove_remote_bundles(
    &self,
    items: impl Into<HashMap<String, ManifestRemoveData>>,
  ) -> crate::Result<Vec<ManifestRemoveResult>> {
    let (results, filepaths) = {
      let mut remote_manifest = self.remote_manifest.write().await;
      let results = remote_manifest.remove_many(items).await?;

      let mut filepaths = Vec::with_capacity(results.len());
      for result in results.iter() {
        if result.kind != ManifestRemoveResultKind::Removed {
          continue;
        }
        let Ok(filepath) = self.get_remote_bundle_filepath(&result.name, &result.version) else {
          continue;
        };
        self.unload_filepath(&result.name, &filepath);
        filepaths.push(filepath);
      }
      (results, filepaths)
    };

    remove_files_by_chunk(filepaths, self.options.remove_bundle_chunk_size).await;

    Ok(results)
  }

  /// Remove orphan remote bundles which is not using so can free disk space.
  pub async fn prune_remote_bundle(&self, bundle_name: &str) -> crate::Result<ManifestPruneResult> {
    let (result, filepaths) = {
      let mut remote_manifest = self.remote_manifest.write().await;
      let result = remote_manifest.prune(bundle_name).await?;

      let mut filepaths = Vec::with_capacity(result.pruned_versions.len());
      for version in result.pruned_versions.iter() {
        if let Ok(filepath) = self.get_remote_bundle_filepath(&result.name, version) {
          filepaths.push(filepath);
        }
      }
      (result, filepaths)
    };

    remove_files_by_chunk(filepaths, self.options.remove_bundle_chunk_size).await;

    Ok(result)
  }

  /// Same as [`Source::prune_remote_bundle`] for several bundles, using a single
  /// manifest write.
  pub async fn prune_remote_bundles<N>(
    &self,
    bundle_names: &[N],
  ) -> crate::Result<Vec<ManifestPruneResult>>
  where
    N: AsRef<str>,
  {
    let (results, filepaths) = {
      let mut remote_manifest = self.remote_manifest.write().await;
      let results = remote_manifest.prune_many(bundle_names).await?;

      let mut filepaths = vec![];
      for result in results.iter() {
        for version in result.pruned_versions.iter() {
          if let Ok(filepath) = self.get_remote_bundle_filepath(&result.name, version) {
            filepaths.push(filepath);
          }
        }
      }
      (results, filepaths)
    };

    remove_files_by_chunk(filepaths, self.options.remove_bundle_chunk_size).await;

    Ok(results)
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

/// Deletes files, best-effort, in batches rather than one task per file.
async fn remove_files_by_chunk(filepaths: Vec<PathBuf>, chunk_size: Option<usize>) {
  if filepaths.is_empty() {
    return;
  }

  for chunk in filepaths.chunks(chunk_size.unwrap_or(256).max(1)) {
    let chunk = chunk.to_vec();
    let _ = tokio::task::spawn_blocking(move || {
      for filepath in chunk {
        let _ = std::fs::remove_file(&filepath);
      }
    })
    .await;
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ChecksumReadOptions;
  use crate::source::{ManifestBundleItemStatus, ManifestBundleSet, ManifestData};
  use crate::testing::{Fixtures, TempDir};
  use std::collections::HashMap;

  fn fixture_source() -> Source {
    let fixture = Fixtures::bundles();
    Source::builder()
      .builtin_dir(fixture.get_path("builtin"))
      .remote_dir(fixture.get_path("remote"))
      .build()
  }

  fn remote_source(temp: &TempDir) -> Source {
    Source::builder()
      .remote_dir(temp.dir().join("remote"))
      .build()
  }

  fn stage_data(version: &str, data: Option<ManifestVersionData>) -> ManifestStageData {
    ManifestStageData {
      version: version.to_owned(),
      data,
    }
  }

  fn remove_data(versions: &[&str], force: Option<bool>) -> ManifestRemoveData {
    ManifestRemoveData {
      versions: versions.iter().map(|x| (*x).to_owned()).collect(),
      force,
    }
  }

  async fn stage(source: &Source, bundle_name: &str, version: &str, data: &[u8]) {
    source
      .stage_remote_bundle(bundle_name, stage_data(version, None))
      .await
      .unwrap();
    let filepath = source
      .get_remote_bundle_filepath(bundle_name, version)
      .unwrap();
    tokio::fs::create_dir_all(filepath.parent().unwrap())
      .await
      .unwrap();
    tokio::fs::write(&filepath, data).await.unwrap();
  }

  async fn staged_source(temp: &TempDir, versions: &[&str]) -> Source {
    let source = remote_source(temp);
    for version in versions {
      stage(&source, "app", version, b"bundle").await;
    }
    source
  }

  async fn staged_bundle_source(temp: &TempDir, versions: &[&str]) -> Source {
    let data = tokio::fs::read(Fixtures::bundles().get_path("remote/app/1.0.0.wvb"))
      .await
      .unwrap();
    let source = remote_source(temp);
    for version in versions {
      stage(&source, "app", version, &data).await;
    }
    source
  }

  #[tokio::test]
  async fn stage_remote_bundle_records_version_data() {
    let temp = TempDir::new();
    let source = remote_source(&temp);
    let data = ManifestVersionData {
      integrity: Some("sha256:abc".to_owned()),
      metadata: Some(HashMap::from([("channel".to_owned(), "stable".to_owned())])),
    };

    source
      .stage_remote_bundle("app", stage_data("1.0.0", Some(data.clone())))
      .await
      .unwrap();

    assert_eq!(
      source
        .get_remote_staged_version("app")
        .await
        .unwrap()
        .as_deref(),
      Some("1.0.0")
    );
    assert_eq!(
      source
        .get_remote_version_data("app", "1.0.0")
        .await
        .unwrap(),
      Some(data.clone())
    );
    assert!(source.get_version("app").await.unwrap().is_none());

    let reopened = remote_source(&temp);
    assert_eq!(
      reopened
        .get_remote_version_data("app", "1.0.0")
        .await
        .unwrap(),
      Some(data)
    );
    assert_eq!(
      reopened
        .get_remote_staged_version("app")
        .await
        .unwrap()
        .as_deref(),
      Some("1.0.0")
    );
  }

  #[tokio::test]
  async fn stage_remote_bundles_stages_every_item() {
    let temp = TempDir::new();
    let source = remote_source(&temp);

    source
      .stage_remote_bundles([
        ("app".to_owned(), stage_data("1.0.0", None)),
        ("docs".to_owned(), stage_data("2.0.0", None)),
      ])
      .await
      .unwrap();

    let reopened = remote_source(&temp);
    let mut items = reopened
      .list_remote_bundles()
      .await
      .unwrap()
      .into_iter()
      .map(|x| (x.item.name, x.item.version, x.item.status))
      .collect::<Vec<_>>();
    items.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    assert_eq!(
      items,
      vec![
        (
          "app".to_owned(),
          "1.0.0".to_owned(),
          ManifestBundleItemStatus::Staged
        ),
        (
          "docs".to_owned(),
          "2.0.0".to_owned(),
          ManifestBundleItemStatus::Staged
        ),
      ]
    );
  }

  #[tokio::test]
  async fn staging_and_updating_nothing_leaves_the_manifest_untouched() {
    let temp = TempDir::new();
    let source = remote_source(&temp);

    source.stage_remote_bundles([]).await.unwrap();
    source.update_remote_versions([]).await.unwrap();

    assert!(!temp.dir().join("remote").join(MANIFEST_FILENAME).exists());
  }

  #[tokio::test]
  async fn update_remote_version_activates_a_staged_version() {
    let temp = TempDir::new();
    let source = staged_source(&temp, &["1.0.0"]).await;

    source.update_remote_version("app", "1.0.0").await.unwrap();

    assert_eq!(
      source.get_version("app").await.unwrap(),
      Some(BundleSourceVersion::remote("1.0.0".to_owned()))
    );
    assert_eq!(source.get_remote_staged_version("app").await.unwrap(), None);

    let reopened = remote_source(&temp);
    assert_eq!(
      reopened.get_version("app").await.unwrap(),
      Some(BundleSourceVersion::remote("1.0.0".to_owned()))
    );
  }

  #[tokio::test]
  async fn update_remote_version_reports_a_version_that_was_never_staged() {
    let temp = TempDir::new();
    let source = staged_source(&temp, &["1.0.0"]).await;

    assert_eq!(
      source.update_remote_version("app", "9.9.9").await.unwrap(),
      ManifestSetCurrentVersionResult::version_not_exists("app", "9.9.9")
    );
    assert_eq!(
      source
        .update_remote_version("unknown", "1.0.0")
        .await
        .unwrap(),
      ManifestSetCurrentVersionResult::not_exists("unknown", "1.0.0")
    );
    assert!(source.get_version("app").await.unwrap().is_none());
  }

  #[tokio::test]
  async fn get_version_prefers_remote_over_builtin() {
    let source = fixture_source();
    assert_eq!(
      source.get_version("app").await.unwrap(),
      Some(BundleSourceVersion::remote("1.0.0".to_owned()))
    );
    assert!(source.get_version("unknown").await.unwrap().is_none());

    let temp = TempDir::new();
    let builtin_only = Source::builder()
      .builtin_dir(Fixtures::bundles().get_path("builtin"))
      .remote_dir(temp.dir().join("remote"))
      .build();
    assert_eq!(
      builtin_only.get_version("app").await.unwrap(),
      Some(BundleSourceVersion::builtin("1.0.0".to_owned()))
    );
  }

  #[tokio::test]
  async fn version_data_is_read_from_the_matching_manifest() {
    let source = fixture_source();

    assert_eq!(
      source
        .get_builtin_version_data("app", "1.0.0")
        .await
        .unwrap(),
      Some(ManifestVersionData::default())
    );
    assert!(
      source
        .get_builtin_version_data("app", "1.1.0")
        .await
        .unwrap()
        .is_none()
    );
    assert_eq!(
      source
        .get_remote_version_data("app", "1.1.0")
        .await
        .unwrap(),
      Some(ManifestVersionData::default())
    );
    assert!(
      source
        .get_remote_version_data("unknown", "1.0.0")
        .await
        .unwrap()
        .is_none()
    );
  }

  #[tokio::test]
  async fn list_bundles_merges_builtin_and_remote_entries() {
    let source = fixture_source();

    let builtin = source.list_builtin_bundles().await.unwrap();
    assert!(builtin.iter().all(|x| x.source == SourceKind::Builtin));
    assert_eq!(
      builtin
        .iter()
        .map(|x| (x.item.version.as_str(), x.item.status.clone()))
        .collect::<Vec<_>>(),
      vec![("1.0.0", ManifestBundleItemStatus::Current)]
    );

    let mut items = source
      .list_bundles()
      .await
      .unwrap()
      .into_iter()
      .map(|x| (x.item.name, x.item.version, x.item.status))
      .collect::<Vec<_>>();
    items.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    assert_eq!(
      items,
      vec![
        (
          "app".to_owned(),
          "1.0.0".to_owned(),
          ManifestBundleItemStatus::Current
        ),
        (
          "app".to_owned(),
          "1.0.0".to_owned(),
          ManifestBundleItemStatus::Current
        ),
        (
          "app".to_owned(),
          "1.1.0".to_owned(),
          ManifestBundleItemStatus::Orphan
        ),
      ]
    );
  }

  #[tokio::test]
  async fn manifest_filepath_options_are_honored() {
    let temp = TempDir::new();
    let builtin_manifest_filepath = temp.dir().join("builtin-manifest.json");
    let mut manifest = ManifestData::default();
    manifest.bundles.insert(
      "app".to_owned(),
      ManifestBundleSet {
        versions: HashMap::from([("1.0.0".to_owned(), ManifestVersionData::default())]),
        current_version: Some("1.0.0".to_owned()),
        previous_version: None,
        staged_version: None,
      },
    );
    tokio::fs::write(
      &builtin_manifest_filepath,
      serde_json::to_vec(&manifest).unwrap(),
    )
    .await
    .unwrap();

    let source = Source::builder()
      .builtin_dir(temp.dir().join("builtin"))
      .builtin_manifest_filepath(&builtin_manifest_filepath)
      .remote_dir(temp.dir().join("remote"))
      .remote_manifest_filepath("custom.json")
      .build();
    source
      .stage_remote_bundle("app", stage_data("2.0.0", None))
      .await
      .unwrap();

    assert!(temp.dir().join("remote").join("custom.json").exists());
    assert!(!temp.dir().join("remote").join(MANIFEST_FILENAME).exists());
    assert_eq!(
      source.get_version("app").await.unwrap(),
      Some(BundleSourceVersion::builtin("1.0.0".to_owned()))
    );
  }

  #[tokio::test]
  async fn resolve_filepath_points_at_the_active_version() {
    let fixture = Fixtures::bundles();
    let source = fixture_source();

    assert_eq!(
      source.resolve_filepath("app").await.unwrap(),
      fixture.get_path("remote").join("app").join("1.0.0.wvb")
    );
    let err = source.resolve_filepath("unknown").await.unwrap_err();
    assert!(matches!(err, crate::Error::BundleNotFound));
  }

  #[tokio::test]
  async fn remove_remote_bundles_deletes_the_files_it_removed() {
    let temp = TempDir::new();
    let source = staged_source(&temp, &["1.0.0", "1.1.0", "1.2.0"]).await;
    source.update_remote_version("app", "1.0.0").await.unwrap();

    let removed = source
      .remove_remote_bundles([("app".to_owned(), remove_data(&["1.1.0", "9.9.9"], None))])
      .await
      .unwrap();
    assert_eq!(
      removed,
      vec![
        ManifestRemoveResult::removed("app", "1.1.0"),
        ManifestRemoveResult::version_not_exists("app", "9.9.9"),
      ]
    );

    let filepath = |version| source.get_remote_bundle_filepath("app", version).unwrap();
    assert!(!filepath("1.1.0").exists());
    assert!(filepath("1.0.0").exists());
    assert!(filepath("1.2.0").exists());

    let reopened = Source::builder()
      .remote_dir(temp.dir().join("remote"))
      .build();
    assert!(
      reopened
        .get_remote_version_data("app", "1.1.0")
        .await
        .unwrap()
        .is_none()
    );
    assert!(
      reopened
        .get_remote_version_data("app", "1.2.0")
        .await
        .unwrap()
        .is_some()
    );
  }

  #[tokio::test]
  async fn remove_remote_bundles_keeps_the_current_version_without_force() {
    let temp = TempDir::new();
    let source = staged_source(&temp, &["1.0.0"]).await;
    source.update_remote_version("app", "1.0.0").await.unwrap();

    let removed = source
      .remove_remote_bundles([("app".to_owned(), remove_data(&["1.0.0"], None))])
      .await
      .unwrap();

    assert_eq!(removed, vec![ManifestRemoveResult::in_use("app", "1.0.0")]);
    assert!(
      source
        .get_remote_bundle_filepath("app", "1.0.0")
        .unwrap()
        .exists()
    );
  }

  #[tokio::test]
  async fn remove_remote_bundle_removes_the_current_version_when_forced() {
    let temp = TempDir::new();
    let source = staged_source(&temp, &["1.0.0"]).await;
    source.update_remote_version("app", "1.0.0").await.unwrap();

    let removed = source
      .remove_remote_bundle("app", "1.0.0", Some(true))
      .await
      .unwrap();

    assert_eq!(removed, ManifestRemoveResult::removed("app", "1.0.0"));
    assert!(
      !source
        .get_remote_bundle_filepath("app", "1.0.0")
        .unwrap()
        .exists()
    );
    assert!(source.get_version("app").await.unwrap().is_none());
  }

  #[tokio::test]
  async fn remove_remote_bundle_deletes_a_single_file() {
    let temp = TempDir::new();
    let source = staged_source(&temp, &["1.0.0", "1.1.0"]).await;
    let filepath = |version| source.get_remote_bundle_filepath("app", version).unwrap();

    let removed = source
      .remove_remote_bundle("app", "1.0.0", None)
      .await
      .unwrap();
    assert_eq!(removed, ManifestRemoveResult::removed("app", "1.0.0"));
    assert!(!filepath("1.0.0").exists());
    assert!(filepath("1.1.0").exists());

    let removed = source
      .remove_remote_bundle("app", "1.0.0", None)
      .await
      .unwrap();
    assert_eq!(
      removed,
      ManifestRemoveResult::version_not_exists("app", "1.0.0")
    );
  }

  #[tokio::test]
  async fn remove_remote_bundles_drops_the_entry_once_its_last_version_is_gone() {
    let temp = TempDir::new();
    let source = staged_source(&temp, &["1.0.0", "1.1.0"]).await;
    let filepath = |version| source.get_remote_bundle_filepath("app", version).unwrap();

    source
      .remove_remote_bundles([("app".to_owned(), remove_data(&["1.0.0"], None))])
      .await
      .unwrap();
    assert_eq!(source.list_remote_bundles().await.unwrap().len(), 1);

    source
      .remove_remote_bundles([("app".to_owned(), remove_data(&["1.1.0"], None))])
      .await
      .unwrap();
    assert!(source.list_remote_bundles().await.unwrap().is_empty());
    assert!(!filepath("1.0.0").exists());
    assert!(!filepath("1.1.0").exists());
  }

  #[tokio::test]
  async fn prune_remote_bundle_returns_and_deletes_only_orphans() {
    let temp = TempDir::new();
    let source = staged_source(&temp, &["1.0.0", "1.1.0", "1.2.0", "1.3.0"]).await;
    source.update_remote_version("app", "1.0.0").await.unwrap();
    source.update_remote_version("app", "1.1.0").await.unwrap();

    let pruned = source.prune_remote_bundle("app").await.unwrap();
    assert_eq!(pruned.name, "app");
    assert_eq!(pruned.pruned_versions, vec!["1.2.0"]);

    let filepath = |version| source.get_remote_bundle_filepath("app", version).unwrap();
    assert!(!filepath("1.2.0").exists());
    for version in ["1.0.0", "1.1.0", "1.3.0"] {
      assert!(filepath(version).exists(), "{version} must be kept");
    }
  }

  #[tokio::test]
  async fn prune_remote_bundles_prunes_every_bundle() {
    let temp = TempDir::new();
    let source = remote_source(&temp);
    for (name, version) in [
      ("a", "1.0.0"),
      ("a", "1.1.0"),
      ("a", "1.2.0"),
      ("b", "2.0.0"),
      ("b", "2.1.0"),
    ] {
      stage(&source, name, version, b"bundle").await;
    }
    source
      .update_remote_versions([
        ("a".to_owned(), "1.0.0".to_owned()),
        ("b".to_owned(), "2.0.0".to_owned()),
      ])
      .await
      .unwrap();

    let mut results = source.prune_remote_bundles(&["a", "b"]).await.unwrap();
    results.sort_by(|x, y| x.name.cmp(&y.name));
    assert_eq!(
      results,
      vec![
        ManifestPruneResult {
          name: "a".to_owned(),
          pruned_versions: vec!["1.1.0".to_owned()],
        },
        ManifestPruneResult {
          name: "b".to_owned(),
          pruned_versions: vec![],
        },
      ]
    );

    let filepath = |name, version| source.get_remote_bundle_filepath(name, version).unwrap();
    assert!(!filepath("a", "1.1.0").exists());
    for (name, version) in [
      ("a", "1.0.0"),
      ("a", "1.2.0"),
      ("b", "2.0.0"),
      ("b", "2.1.0"),
    ] {
      assert!(
        filepath(name, version).exists(),
        "{name} {version} must be kept"
      );
    }

    let reopened = remote_source(&temp);
    assert!(
      reopened
        .get_remote_version_data("a", "1.1.0")
        .await
        .unwrap()
        .is_none()
    );
  }

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
      "con",
      "CON",
      "NuL",
      "com1",
      "LPT9",
      "aux",
      "prn",
      "nul.txt",
      "con.foo.bar",
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
    let source = Source::builder()
      .builtin_dir("/tmp/builtin")
      .remote_dir("/tmp/remote")
      .build();

    assert!(source.get_remote_bundle_filepath("app", "1.0.0").is_ok());
    assert!(
      source
        .get_builtin_bundle_filepath("my-app", "1.2.3-beta.4")
        .is_ok()
    );

    for name in ["", "..", "a/b", "../etc", "a b"] {
      assert!(
        matches!(
          source.get_remote_bundle_filepath(name, "1.0.0"),
          Err(crate::Error::InvalidFilepath(_))
        ),
        "name {name:?} should be rejected"
      );
    }

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
  async fn invalid_filepath_when_fetch_remote_bundle() {
    let fixture = Fixtures::bundles();
    let source = Source::builder()
      .remote_dir(fixture.get_path("remote"))
      .build();
    let err = source
      .fetch_remote_bundle("../evil", "1.0.0")
      .await
      .unwrap_err();
    assert!(matches!(err, crate::Error::InvalidFilepath(_)));
  }

  #[tokio::test]
  async fn fetch() {
    let source = fixture_source();
    let bundle = source.fetch_bundle("app").await.unwrap();
    bundle.get_data("/index.html").unwrap().unwrap();
  }

  #[tokio::test]
  async fn fetch_bundle_of_an_explicit_version() {
    let source = fixture_source();

    source
      .fetch_builtin_bundle("app", "1.0.0")
      .await
      .unwrap()
      .get_data("/index.html")
      .unwrap()
      .unwrap();
    source
      .fetch_remote_bundle("app", "1.1.0")
      .await
      .unwrap()
      .get_data("/index.html")
      .unwrap()
      .unwrap();

    let err = source
      .fetch_remote_bundle("app", "9.9.9")
      .await
      .unwrap_err();
    assert!(matches!(err, crate::Error::BundleNotFound));
  }

  #[tokio::test]
  async fn fetch_descriptor() {
    let source = fixture_source();
    let descriptor = source.fetch_descriptor("app").await.unwrap();
    assert!(descriptor.index().contains_path("/index.html"));
  }

  #[tokio::test]
  async fn fetch_many_times() {
    let source = Arc::new(fixture_source());
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
    let source = fixture_source();
    let bundle = source.fetch_bundle("not-found").await;
    assert!(matches!(bundle.unwrap_err(), crate::Error::BundleNotFound));
  }

  #[tokio::test]
  async fn load_reads_data_of_the_loaded_version() {
    let source = fixture_source();
    let loaded = source.load("app").await.unwrap();

    assert!(loaded.index().contains_path("/index.html"));
    assert!(
      !loaded
        .get_data("/index.html")
        .await
        .unwrap()
        .unwrap()
        .is_empty()
    );
    assert!(loaded.get_data("/not-exists.html").await.unwrap().is_none());
    assert!(
      loaded
        .get_data_checksum("/index.html")
        .await
        .unwrap()
        .is_some()
    );
    assert!(
      loaded
        .get_data_checksum("/not-exists.html")
        .await
        .unwrap()
        .is_none()
    );
  }

  #[tokio::test]
  async fn load_applies_the_data_read_options_of_the_source() {
    let fixture = Fixtures::bundles();
    let data_read = DataReadOptions::default().checksum(ChecksumReadOptions::default().seed(42));
    let source = Source::builder()
      .builtin_dir(fixture.get_path("builtin"))
      .remote_dir(fixture.get_path("remote"))
      .options(SourceOptions::default().data_read(data_read))
      .build();
    assert_eq!(source.options().data_read, data_read);

    let loaded = source.load("app").await.unwrap();
    assert_eq!(loaded.data_read_options(), &data_read);

    let err = loaded.get_data("/index.html").await.unwrap_err();
    assert!(matches!(err, crate::Error::ChecksumMismatch));

    let unverified =
      DataReadOptions::default().checksum(ChecksumReadOptions::default().verify(false));
    loaded
      .get_data_with_options("/index.html", unverified)
      .await
      .unwrap()
      .unwrap();
  }

  #[tokio::test]
  async fn load_reloads_when_the_active_version_changes() {
    let temp = TempDir::new();
    let source = staged_bundle_source(&temp, &["1.0.0", "1.1.0"]).await;
    source.update_remote_version("app", "1.0.0").await.unwrap();

    let first = source.load("app").await.unwrap();
    assert_eq!(
      first.filepath,
      source.get_remote_bundle_filepath("app", "1.0.0").unwrap()
    );
    let cached = source.load("app").await.unwrap();
    assert!(Arc::ptr_eq(first.descriptor(), cached.descriptor()));

    source.update_remote_version("app", "1.1.0").await.unwrap();

    let next = source.load("app").await.unwrap();
    assert!(!Arc::ptr_eq(first.descriptor(), next.descriptor()));
    assert_eq!(
      next.filepath,
      source.get_remote_bundle_filepath("app", "1.1.0").unwrap()
    );
  }

  #[tokio::test]
  async fn remove_remote_bundle_unloads_only_the_version_it_removed() {
    let temp = TempDir::new();
    let source = staged_bundle_source(&temp, &["1.0.0", "1.1.0"]).await;
    source.update_remote_version("app", "1.0.0").await.unwrap();
    let loaded = source.load("app").await.unwrap();

    source
      .remove_remote_bundle("app", "1.1.0", None)
      .await
      .unwrap();
    let after = source.load("app").await.unwrap();
    assert!(Arc::ptr_eq(loaded.descriptor(), after.descriptor()));

    source
      .remove_remote_bundle("app", "1.0.0", Some(true))
      .await
      .unwrap();
    assert!(!source.descriptors.contains_key("app"));
    assert!(matches!(
      source.load("app").await.unwrap_err(),
      crate::Error::BundleNotFound
    ));
  }

  #[tokio::test]
  async fn load_many_at_once() {
    let source = Arc::new(fixture_source());
    let mut handles = Vec::new();
    for _i in 0..10 {
      let s = source.clone();
      let handle = tokio::spawn(async move {
        s.load("app").await.unwrap();
      });
      handles.push(handle);
    }
    for h in handles {
      h.await.unwrap();
    }
  }

  #[tokio::test]
  async fn load_and_unload_sequential() {
    let source = Arc::new(fixture_source());
    assert!(!source.unload("app"), "nothing is loaded yet");

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
    use tokio::sync::Barrier;
    use tokio::task::JoinSet;

    let source = Arc::new(fixture_source());

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
    for m in &before_jobs {
      assert!(Arc::ptr_eq(initials[0].descriptor(), m.descriptor()));
    }
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
