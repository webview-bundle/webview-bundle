use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, OnceCell, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
  feature = "_serde",
  derive(serde_repr::Serialize_repr, serde_repr::Deserialize_repr)
)]
#[cfg_attr(feature = "_serde", repr(u8))]
pub enum BundleManifestVersion {
  #[default]
  V1 = 1,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct BundleManifestMetadata {
  pub etag: Option<String>,
  pub integrity: Option<String>,
  pub signature: Option<String>,
  pub last_modified: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct BundleManifestEntry {
  pub versions: HashMap<String, BundleManifestMetadata>,
  /// The active version, or `None` when versions are present on disk but none has
  /// been activated yet. A download stages a version (adds it to `versions`) without
  /// activating it; activation happens only via `update_current_version`.
  #[cfg_attr(
    feature = "_serde",
    serde(default, skip_serializing_if = "Option::is_none")
  )]
  pub current_version: Option<String>,
  /// The version that was active immediately before `current_version`. Tracked so a
  /// just-activated update can clean up older bundles while keeping the previous one
  /// on disk — it may still be referenced by an in-flight protocol request, and it is
  /// the target of a one-step rollback. `None` before the first activation swap.
  #[cfg_attr(
    feature = "_serde",
    serde(default, skip_serializing_if = "Option::is_none")
  )]
  pub previous_version: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub struct BundleManifestData {
  pub manifest_version: BundleManifestVersion,
  pub entries: HashMap<String, BundleManifestEntry>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub struct ListBundleManifestItem {
  pub name: String,
  pub version: String,
  pub current: bool,
  pub metadata: BundleManifestMetadata,
}

pub trait BundleManifestMode: Send + Sync + 'static {}

#[derive(Debug)]
pub struct ReadOnly;
impl BundleManifestMode for ReadOnly {}

#[derive(Debug)]
pub struct ReadWrite;
impl BundleManifestMode for ReadWrite {}

#[derive(Debug)]
pub struct BundleManifest<Mode: BundleManifestMode> {
  _mode: std::marker::PhantomData<Mode>,
  filepath: PathBuf,
  data: OnceCell<RwLock<BundleManifestData>>,
  // Serializes `save()` end-to-end (snapshot + write + rename). The RwLock keeps the
  // in-memory data consistent, but without this two concurrent saves (e.g. installs of
  // different bundles, which take different per-bundle locks yet share this one manifest
  // file) could have their renames reorder, persisting an older snapshot last and losing
  // a just-activated version on disk. Holding this across snapshot+rename forces the
  // latest snapshot to land last.
  save_lock: Mutex<()>,
}

impl<Mode> BundleManifest<Mode>
where
  Mode: BundleManifestMode,
{
  pub fn new(filepath: &Path, _mode: Mode) -> Self {
    Self {
      _mode: std::marker::PhantomData,
      filepath: filepath.to_path_buf(),
      data: Default::default(),
      save_lock: Mutex::new(()),
    }
  }

  pub async fn list_entries(&self) -> crate::Result<Vec<ListBundleManifestItem>> {
    let data = self.load().await?.read().await;
    let mut items = vec![];
    for (bundle_name, entry) in data.entries.iter() {
      let current_version = entry.current_version.as_deref();
      for (version, metadata) in entry.versions.iter() {
        let item = ListBundleManifestItem {
          name: bundle_name.to_string(),
          version: version.to_string(),
          current: current_version == Some(version.as_str()),
          metadata: metadata.clone(),
        };
        items.push(item);
      }
    }
    Ok(items)
  }

  pub async fn contains_entry(&self, bundle_name: &str, version: &str) -> crate::Result<bool> {
    let data = self.load().await?.read().await;
    if let Some(entry) = data.entries.get(bundle_name) {
      return Ok(entry.versions.contains_key(version));
    }
    Ok(false)
  }

  pub async fn load_current_version(&self, bundle_name: &str) -> crate::Result<Option<String>> {
    let data = self.load().await?.read().await;
    let version = data
      .entries
      .get(bundle_name)
      .and_then(|x| x.current_version.to_owned());
    Ok(version)
  }

  /// All versions recorded for `bundle_name` (in unspecified order).
  pub async fn list_versions(&self, bundle_name: &str) -> crate::Result<Vec<String>> {
    let data = self.load().await?.read().await;
    let versions = data
      .entries
      .get(bundle_name)
      .map(|entry| entry.versions.keys().cloned().collect())
      .unwrap_or_default();
    Ok(versions)
  }

  pub async fn load_current_metadata(
    &self,
    bundle_name: &str,
  ) -> crate::Result<Option<BundleManifestMetadata>> {
    let version = self.load_current_version(bundle_name).await?;
    if let Some(ver) = version {
      let metadata = self.load_metadata(bundle_name, &ver).await?;
      return Ok(metadata);
    }
    Ok(None)
  }

  pub async fn load_metadata(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Option<BundleManifestMetadata>> {
    let data = self.load().await?.read().await;
    let metadata = data
      .entries
      .get(bundle_name)
      .and_then(|entry| entry.versions.get(version))
      .cloned();
    Ok(metadata)
  }

  async fn load(&self) -> crate::Result<&RwLock<BundleManifestData>> {
    let data = self
      .data
      .get_or_try_init(|| async {
        // Existence is probed by the read itself: a separate `try_exists` sits outside the
        // Windows retry below, and metadata queries can also transiently fail with
        // ACCESS_DENIED while a concurrent save renames the manifest into place.
        let raw = match read_with_win_retry(&self.filepath).await {
          Ok(raw) => raw,
          Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok::<RwLock<BundleManifestData>, crate::Error>(Default::default());
          }
          Err(e) => return Err(e.into()),
        };
        let data: BundleManifestData = serde_json::from_slice(&raw)?;
        Ok::<RwLock<BundleManifestData>, crate::Error>(RwLock::new(data))
      })
      .await?;
    Ok(data)
  }
}

// Windows briefly returns ACCESS_DENIED (5) / SHARING_VIOLATION (32) when reading a file a concurrent save is renaming into place; retry those.
async fn read_with_win_retry(path: &Path) -> std::io::Result<Vec<u8>> {
  let mut attempts = 0;
  loop {
    match tokio::fs::read(path).await {
      Err(e)
        if attempts < 20 && cfg!(windows) && matches!(e.raw_os_error(), Some(5) | Some(32)) =>
      {
        attempts += 1;
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
      }
      result => return result,
    }
  }
}

static SAVE_SEQ: AtomicU64 = AtomicU64::new(0);

impl BundleManifest<ReadWrite> {
  pub async fn update_current_version(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<()> {
    // Check existence and mutate under a single write lock. Splitting the existence
    // check (a separate read lock) from the mutation would let a concurrent removal
    // slip in between, leaving current_version pointing at a version no longer staged.
    let mut data = self.load().await?.write().await;
    let entry = data
      .entries
      .get_mut(bundle_name)
      .ok_or_else(|| crate::Error::bundle_entry_not_exists(bundle_name, version))?;
    if !entry.versions.contains_key(version) {
      return Err(crate::Error::bundle_entry_not_exists(bundle_name, version));
    }
    // Activating a different version demotes the old current to previous, so the old
    // bundle is retained (in-flight requests / one-step rollback) while older versions
    // become eligible for cleanup. Re-activating the same version is a no-op and leaves
    // `previous_version` intact.
    if entry.current_version.as_deref() != Some(version) {
      entry.previous_version = entry.current_version.take();
      entry.current_version = Some(version.to_string());
    }
    Ok(())
  }

  /// Returns the versions that must be retained for `bundle_name`: the active version
  /// and the immediately-previous one. Any other version is safe to delete from disk.
  pub async fn retained_versions(&self, bundle_name: &str) -> crate::Result<Vec<String>> {
    let data = self.load().await?.read().await;
    let mut retained = vec![];
    if let Some(entry) = data.entries.get(bundle_name) {
      if let Some(current) = &entry.current_version {
        retained.push(current.clone());
      }
      if let Some(previous) = &entry.previous_version {
        if !retained.contains(previous) {
          retained.push(previous.clone());
        }
      }
    }
    Ok(retained)
  }

  pub async fn insert_entry(
    &self,
    bundle_name: &str,
    version: &str,
    metadata: BundleManifestMetadata,
  ) -> crate::Result<bool> {
    let mut inserted = true;
    let mut data = self.load().await?.write().await;
    data
      .entries
      .entry(bundle_name.to_string())
      .and_modify(|entry| {
        if entry.versions.contains_key(version) {
          inserted = false;
        } else {
          entry.versions.insert(version.to_string(), metadata.clone());
        }
      })
      .or_insert_with(|| BundleManifestEntry {
        versions: HashMap::from([(version.to_string(), metadata.clone())]),
        current_version: None,
        previous_version: None,
      });
    Ok(inserted)
  }

  pub async fn remove_entry(&self, bundle_name: &str, version: &str) -> crate::Result<bool> {
    let mut data = self.load().await?.write().await;
    if let Some(entry) = data.entries.get_mut(bundle_name) {
      if entry.current_version.as_deref() == Some(version) {
        return Err(crate::Error::bundle_cannot_be_removed(
          bundle_name,
          version,
          "current version of bundle cannot be removed",
        ));
      }
      // Don't leave `previous_version` dangling at a version we just removed.
      if entry.previous_version.as_deref() == Some(version) {
        entry.previous_version = None;
      }
      return Ok(entry.versions.remove(version).is_some());
    }
    Ok(false)
  }

  pub async fn save(&self) -> crate::Result<()> {
    let _save = self.save_lock.lock().await;
    let raw = {
      let data = self.load().await?.read().await;
      serde_json::to_vec(&*data)
    }?;

    if let Some(dir) = self.filepath.parent() {
      tokio::fs::create_dir_all(dir).await?;
    }

    // Write to a temp file then atomically rename into place.
    let seq = SAVE_SEQ.fetch_add(1, Ordering::Relaxed);

    let mut tmp = self.filepath.clone().into_os_string();
    tmp.push(format!(".{seq}.tmp"));

    let tmp = PathBuf::from(tmp);
    tokio::fs::write(&tmp, raw).await?;

    if let Err(e) = tokio::fs::rename(&tmp, &self.filepath).await {
      let _ = tokio::fs::remove_file(&tmp).await;
      return Err(e.into());
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::testing::*;
  use std::sync::Arc;

  #[tokio::test]
  async fn list_entries() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadOnly);
    let items = manifest.list_entries().await.unwrap();
    assert_eq!(items.len(), 2);
    let current = items.iter().find(|x| x.name == "app" && x.current).unwrap();
    assert_eq!(current.version, "1.0.0");
  }

  #[tokio::test]
  async fn load_metadata() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("builtin/manifest.json"), ReadOnly);
    manifest
      .load_metadata("app", "1.0.0")
      .await
      .unwrap()
      .unwrap();
    assert!(
      manifest
        .load_metadata("app", "not_exists")
        .await
        .unwrap()
        .is_none()
    );
  }

  #[tokio::test]
  async fn load_metadata_many_times() {
    let fixture = Fixtures::bundles();
    let manifest = Arc::new(BundleManifest::new(
      &fixture.get_path("builtin/manifest.json"),
      ReadOnly,
    ));
    let mut handlers = vec![];
    for _ in 1..10 {
      let m = manifest.clone();
      let handle = tokio::spawn(async move { m.load_metadata("app", "1.0.0").await });
      handlers.push(handle);
    }
    for h in handlers {
      h.await.unwrap().unwrap();
    }
  }

  #[tokio::test]
  async fn load_current_version() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadOnly);
    let version = manifest.load_current_version("app").await.unwrap().unwrap();
    assert_eq!(version, "1.0.0");
  }

  #[tokio::test]
  async fn load_current_version_many_times() {
    let fixture = Fixtures::bundles();
    let manifest = Arc::new(BundleManifest::new(
      &fixture.get_path("remote/manifest.json"),
      ReadOnly,
    ));
    let mut handlers = vec![];
    for _ in 1..10 {
      let m = manifest.clone();
      let handle = tokio::spawn(async move { m.load_current_version("app").await });
      handlers.push(handle);
    }
    for h in handlers {
      let version = h.await.unwrap().unwrap().unwrap();
      assert_eq!(version, "1.0.0");
    }
  }

  #[tokio::test]
  async fn update_current_version() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadWrite);
    manifest
      .update_current_version("app", "1.1.0")
      .await
      .unwrap();
    assert_eq!(
      manifest.load_current_version("app").await.unwrap().unwrap(),
      "1.1.0"
    );
  }

  #[tokio::test]
  async fn update_current_version_tracks_previous_and_retained() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadWrite);
    // Fixture starts at current = 1.0.0, previous = None.
    assert_eq!(
      manifest.retained_versions("app").await.unwrap(),
      vec!["1.0.0"]
    );

    // Activating 1.1.0 demotes 1.0.0 to previous; both are retained.
    manifest
      .update_current_version("app", "1.1.0")
      .await
      .unwrap();
    let retained = manifest.retained_versions("app").await.unwrap();
    assert_eq!(retained, vec!["1.1.0".to_string(), "1.0.0".to_string()]);

    // Re-activating the same version is a no-op and keeps previous intact.
    manifest
      .update_current_version("app", "1.1.0")
      .await
      .unwrap();
    assert_eq!(
      manifest.retained_versions("app").await.unwrap(),
      vec!["1.1.0".to_string(), "1.0.0".to_string()]
    );
  }

  #[tokio::test]
  async fn update_current_version_entry_not_exists() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadWrite);
    let err = manifest
      .update_current_version("app", "not_exists")
      .await
      .unwrap_err();
    assert_eq!(
      err.to_string(),
      "bundle entry not exists (bundle_name: app, version: not_exists)"
    );
    let err = manifest
      .update_current_version("not_exists", "1.0.0")
      .await
      .unwrap_err();
    assert_eq!(
      err.to_string(),
      "bundle entry not exists (bundle_name: not_exists, version: 1.0.0)"
    );
  }

  #[tokio::test]
  async fn insert_entry() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadWrite);
    let metadata = BundleManifestMetadata {
      etag: None,
      integrity: None,
      signature: None,
      last_modified: None,
    };
    let inserted = manifest
      .insert_entry("app", "1.2.0", metadata.clone())
      .await
      .unwrap();
    assert!(inserted);
    assert_eq!(
      manifest
        .load_metadata("app", "1.2.0")
        .await
        .unwrap()
        .unwrap(),
      metadata
    );
    // Staging a new version into an existing entry must NOT change the active version.
    assert_eq!(
      manifest.load_current_version("app").await.unwrap().unwrap(),
      "1.0.0"
    );
  }

  #[tokio::test]
  async fn insert_entry_from_empty() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(
      &fixture.get_path("bundles").join("manifest.json"),
      ReadWrite,
    );
    let metadata = BundleManifestMetadata {
      etag: None,
      integrity: None,
      signature: None,
      last_modified: None,
    };
    let inserted = manifest
      .insert_entry("vite", "1.0.0", metadata.clone())
      .await
      .unwrap();
    assert!(inserted);
    assert_eq!(
      manifest
        .load_metadata("vite", "1.0.0")
        .await
        .unwrap()
        .unwrap(),
      metadata
    );
    // A freshly inserted version is staged on disk, not yet activated.
    assert!(
      manifest
        .load_current_version("vite")
        .await
        .unwrap()
        .is_none(),
      "insert_entry must not activate the version"
    );
    // Activation is a separate, explicit step.
    manifest
      .update_current_version("vite", "1.0.0")
      .await
      .unwrap();
    assert_eq!(
      manifest
        .load_current_version("vite")
        .await
        .unwrap()
        .unwrap(),
      "1.0.0"
    );
  }

  #[tokio::test]
  async fn remove_entry() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadWrite);
    let removed = manifest.remove_entry("app", "1.1.0").await.unwrap();
    assert!(removed);
    assert!(
      manifest
        .load_metadata("app", "1.1.0")
        .await
        .unwrap()
        .is_none()
    );
  }

  #[tokio::test]
  async fn remove_entry_current_version_cannot_be_removed() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadWrite);
    manifest
      .update_current_version("app", "1.1.0")
      .await
      .unwrap();
    let err = manifest.remove_entry("app", "1.1.0").await.unwrap_err();
    assert_eq!(
      err.to_string(),
      "bundle cannot be removed (bundle_name: app, version: 1.1.0): current version of bundle cannot be removed"
    );
  }

  #[tokio::test]
  async fn remove_entry_clears_previous_pointer() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadWrite);
    // current = 1.1.0, previous = 1.0.0
    manifest
      .update_current_version("app", "1.1.0")
      .await
      .unwrap();
    assert_eq!(
      manifest.retained_versions("app").await.unwrap(),
      vec!["1.1.0".to_string(), "1.0.0".to_string()]
    );
    // Removing the previous version drops it from retained and clears the dangling pointer.
    assert!(manifest.remove_entry("app", "1.0.0").await.unwrap());
    assert_eq!(
      manifest.retained_versions("app").await.unwrap(),
      vec!["1.1.0".to_string()]
    );
  }
}
