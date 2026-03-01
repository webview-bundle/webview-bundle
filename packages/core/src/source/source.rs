use crate::source::{
  BundleManifest, BundleManifestMetadata, ListBundleManifestItem, ReadOnly, ReadWrite, utils,
};
use crate::{
  AsyncBundleReader, AsyncBundleWriter, AsyncReader, AsyncWriter, Bundle, BundleDescriptor,
  EXTENSION, MANIFEST_FILENAME,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::File;
use tokio::sync::OnceCell;

/// The type of bundle source: builtin or remote.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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
pub struct BundleSourceVersion {
  /// The source kind (builtin or remote)
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
#[non_exhaustive]
pub struct ListBundleItem {
  pub kind: BundleSourceKind,
  pub item: ListBundleManifestItem,
}

#[derive(Debug)]
pub struct BundleSource {
  builtin_dir: PathBuf,
  builtin_manifest: BundleManifest<ReadOnly>,
  remote_dir: PathBuf,
  remote_manifest: BundleManifest<ReadWrite>,
  descriptors: DashMap<String, Arc<OnceCell<Arc<BundleDescriptor>>>>,
}

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

  pub async fn update_version(&self, bundle_name: &str, version: &str) -> crate::Result<()> {
    self
      .remote_manifest
      .update_current_version(bundle_name, version)
      .await
  }

  pub async fn filepath(&self, bundle_name: &str) -> crate::Result<PathBuf> {
    let ver = self
      .load_version(bundle_name)
      .await?
      .ok_or(crate::Error::BundleNotFound)?;
    let filepath = match &ver.kind {
      BundleSourceKind::Builtin => self.get_builtin_filepath(bundle_name, &ver.version),
      BundleSourceKind::Remote => self.get_remote_filepath(bundle_name, &ver.version),
    };
    Ok(filepath)
  }

  pub async fn reader(&self, bundle_name: &str) -> crate::Result<File> {
    let filepath = self.filepath(bundle_name).await?;
    let file = File::open(filepath).await.map_err(|e| {
      if e.kind() == std::io::ErrorKind::NotFound {
        return crate::Error::BundleNotFound;
      }
      crate::Error::from(e)
    })?;
    Ok(file)
  }

  pub async fn fetch(&self, bundle_name: &str) -> crate::Result<Bundle> {
    let mut file = self.reader(bundle_name).await?;
    let bundle = AsyncReader::<Bundle>::read(&mut AsyncBundleReader::new(&mut file)).await?;
    Ok(bundle)
  }

  pub async fn fetch_descriptor(&self, bundle_name: &str) -> crate::Result<BundleDescriptor> {
    let mut file = self.reader(bundle_name).await?;
    let manifest =
      AsyncReader::<BundleDescriptor>::read(&mut AsyncBundleReader::new(&mut file)).await?;
    Ok(manifest)
  }

  pub async fn load_descriptor(&self, bundle_name: &str) -> crate::Result<Arc<BundleDescriptor>> {
    if let Some(entry) = self.descriptors.get(bundle_name) {
      if let Some(m) = entry.get() {
        return Ok(m.clone());
      }
    }
    let descriptor_cell = {
      let entry = self.descriptors.entry(bundle_name.to_string()).or_default();
      entry.clone()
    };
    let descriptor = descriptor_cell
      .get_or_try_init(|| async {
        let d = self.fetch_descriptor(bundle_name).await?;
        Ok::<Arc<BundleDescriptor>, crate::Error>(Arc::new(d))
      })
      .await?
      .clone();
    Ok(descriptor)
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
    let filepath = self.get_remote_filepath(bundle_name, version);
    if let Some(parent) = filepath.parent() {
      let _ = tokio::fs::create_dir_all(parent).await;
    }
    let mut file = File::create(&filepath).await?;
    AsyncBundleWriter::new(&mut file).write(bundle).await?;
    self
      .remote_manifest
      .insert_entry(bundle_name, version, metadata)
      .await?;
    Ok(())
  }

  fn get_builtin_filepath(&self, bundle_name: &str, version: &str) -> PathBuf {
    self.get_filepath(&self.builtin_dir, bundle_name, version)
  }

  fn get_remote_filepath(&self, bundle_name: &str, version: &str) -> PathBuf {
    self.get_filepath(&self.remote_dir, bundle_name, version)
  }

  fn get_filepath(&self, base_dir: &Path, bundle_name: &str, version: &str) -> PathBuf {
    // TODO: normalize bundle name
    let filename = format!("{bundle_name}_{version}.{EXTENSION}");
    base_dir.join(bundle_name).join(filename)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::testing::Fixtures;

  #[tokio::test]
  async fn fetch() {
    let fixture = Fixtures::bundles();
    let source = BundleSource::builder()
      .builtin_dir(fixture.get_path("builtin"))
      .remote_dir(fixture.get_path("remote"))
      .build();
    let bundle = source.fetch("app").await.unwrap();
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
    let reader = source.reader("app").await.unwrap();
    descriptor
      .async_get_data(reader, "/index.html")
      .await
      .unwrap()
      .unwrap();
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
        let bundle = s.fetch("app").await.unwrap();
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
    let bundle = source.fetch("not-found").await;
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
      !Arc::ptr_eq(&m1, &m2),
      "after unload, reloading should produce a new Arc"
    );

    assert!(source.unload_descriptor("app"));
    let m3 = source.load_descriptor("app").await.unwrap();
    assert!(!Arc::ptr_eq(&m2, &m3));

    assert!(source.unload_descriptor("app"));
    let m4 = source.load_descriptor("app").await.unwrap();
    assert!(!Arc::ptr_eq(&m3, &m4));
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
      assert!(Arc::ptr_eq(&initials[0], m));
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
      assert!(Arc::ptr_eq(&initials[0], m));
    }
    // after jobs should be not same with initial loads
    for m in &after_jobs {
      assert!(!Arc::ptr_eq(&initials[0], m));
    }
    for m in &before_jobs[1..] {
      assert!(Arc::ptr_eq(&before_jobs[0], m));
    }
    for m in &after_jobs[1..] {
      assert!(Arc::ptr_eq(&after_jobs[0], m));
    }
  }
}
