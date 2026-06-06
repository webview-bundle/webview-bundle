use crate::source::{
  BundleManifest, BundleManifestMetadata, ListBundleManifestItem, ReadOnly, ReadWrite, utils,
};
use crate::{
  AsyncBundleReader, AsyncBundleWriter, AsyncReader, AsyncWriter, Bundle, BundleDescriptor,
  EXTENSION, MANIFEST_FILENAME,
};
use dashmap::DashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::File;
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
}

impl LoadedDescriptor {
  pub async fn reader(&self) -> crate::Result<File> {
    open_file(&self.filepath).await
  }

  pub fn descriptor(&self) -> &Arc<BundleDescriptor> {
    &self.descriptor
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
    let ver = self
      .load_version(bundle_name)
      .await?
      .ok_or(crate::Error::BundleNotFound)?;
    let filepath = match &ver.kind {
      BundleSourceKind::Builtin => self.get_builtin_bundle_filepath(bundle_name, &ver.version)?,
      BundleSourceKind::Remote => self.get_remote_bundle_filepath(bundle_name, &ver.version)?,
    };
    Ok(filepath)
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
    let filepath = self.resolve_filepath(bundle_name).await?;
    let mut file = open_file(&filepath).await?;
    let bundle = AsyncReader::<Bundle>::read(&mut AsyncBundleReader::new(&mut file)).await?;
    Ok(bundle)
  }

  pub async fn fetch_builtin_bundle(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Bundle> {
    let filepath = self.get_builtin_bundle_filepath(bundle_name, version)?;
    let mut file = open_file(&filepath).await?;
    let bundle = AsyncReader::<Bundle>::read(&mut AsyncBundleReader::new(&mut file)).await?;
    Ok(bundle)
  }

  pub async fn fetch_remote_bundle(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Bundle> {
    let filepath = self.get_remote_bundle_filepath(bundle_name, version)?;
    let mut file = open_file(&filepath).await?;
    let bundle = AsyncReader::<Bundle>::read(&mut AsyncBundleReader::new(&mut file)).await?;
    Ok(bundle)
  }

  pub async fn fetch_descriptor(&self, bundle_name: &str) -> crate::Result<BundleDescriptor> {
    let filepath = self.resolve_filepath(bundle_name).await?;
    let mut file = open_file(&filepath).await?;
    let manifest =
      AsyncReader::<BundleDescriptor>::read(&mut AsyncBundleReader::new(&mut file)).await?;
    Ok(manifest)
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
    let filepath = self.resolve_filepath(bundle_name).await?;
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
    let descriptor = cell
      .get_or_try_init(|| {
        let filepath = filepath.clone();
        async move {
          let mut file = open_file(&filepath).await?;
          let d =
            AsyncReader::<BundleDescriptor>::read(&mut AsyncBundleReader::new(&mut file)).await?;
          Ok::<Arc<BundleDescriptor>, crate::Error>(Arc::new(d))
        }
      })
      .await?
      .clone();
    Ok(Arc::new(LoadedDescriptor {
      descriptor,
      filepath,
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

    AsyncBundleWriter::new(&mut file).write(bundle).await?;
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

async fn open_file(filepath: &Path) -> crate::Result<File> {
  File::open(filepath).await.map_err(|e| {
    if e.kind() == std::io::ErrorKind::NotFound {
      return crate::Error::BundleNotFound;
    }
    crate::Error::from(e)
  })
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
