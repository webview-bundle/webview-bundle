use crate::util;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
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
pub struct BundleManifestVersionData {
  pub integrity: Option<String>,
  pub metadata: Option<HashMap<String, String>>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct BundleManifestEntry {
  pub versions: HashMap<String, BundleManifestVersionData>,
  /// The current version, or `None` when versions are present on disk but none has
  /// been activated yet.
  #[cfg_attr(
    feature = "_serde",
    serde(default, skip_serializing_if = "Option::is_none")
  )]
  pub current_version: Option<String>,
  /// The previous version that was recorded before the current version changed.
  #[cfg_attr(
    feature = "_serde",
    serde(default, skip_serializing_if = "Option::is_none")
  )]
  pub previous_version: Option<String>,
  /// The staged version that has been downloaded from remote.
  #[cfg_attr(
    feature = "_serde",
    serde(default, skip_serializing_if = "Option::is_none")
  )]
  pub staged_version: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub struct BundleManifestData {
  pub manifest_version: BundleManifestVersion,
  pub entries: HashMap<String, BundleManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub enum BundleManifestEntryItemStatus {
  Current,
  Previous,
  Staged,
  Orphan,
}

impl BundleManifestEntryItemStatus {
  pub(crate) fn from(entry: &BundleManifestEntry, version: &str) -> Self {
    if let Some(current_version) = entry.current_version.as_deref()
      && current_version == version
    {
      BundleManifestEntryItemStatus::Current
    } else if let Some(previous_version) = entry.previous_version.as_deref()
      && previous_version == version
    {
      BundleManifestEntryItemStatus::Previous
    } else if let Some(staged_version) = entry.staged_version.as_deref()
      && staged_version == version
    {
      BundleManifestEntryItemStatus::Staged
    } else {
      BundleManifestEntryItemStatus::Orphan
    }
  }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub struct BundleManifestEntryItem {
  pub name: String,
  pub version: String,
  pub status: BundleManifestEntryItemStatus,
  pub data: BundleManifestVersionData,
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

  pub async fn list_entries(&self) -> crate::Result<Vec<BundleManifestEntryItem>> {
    let manifest = self.load().await?.read().await;
    let mut items = vec![];
    for (bundle_name, entry) in manifest.entries.iter() {
      for (version, data) in entry.versions.iter() {
        let item = BundleManifestEntryItem {
          name: bundle_name.to_string(),
          version: version.to_string(),
          status: BundleManifestEntryItemStatus::from(entry, version),
          data: data.clone(),
        };
        items.push(item);
      }
    }
    Ok(items)
  }

  pub async fn contains_entry(&self, bundle_name: &str, version: &str) -> crate::Result<bool> {
    let manifest = self.load().await?.read().await;
    if let Some(entry) = manifest.entries.get(bundle_name) {
      return Ok(entry.versions.contains_key(version));
    }
    Ok(false)
  }

  pub async fn get_current_version(&self, bundle_name: &str) -> crate::Result<Option<String>> {
    let manifest = self.load().await?.read().await;
    let version = manifest
      .entries
      .get(bundle_name)
      .and_then(|x| x.current_version.to_owned());
    Ok(version)
  }

  pub async fn get_previous_version(&self, bundle_name: &str) -> crate::Result<Option<String>> {
    let manifest = self.load().await?.read().await;
    let version = manifest
      .entries
      .get(bundle_name)
      .and_then(|x| x.previous_version.to_owned());
    Ok(version)
  }

  pub async fn get_staged_version(&self, bundle_name: &str) -> crate::Result<Option<String>> {
    let manifest = self.load().await?.read().await;
    let version = manifest
      .entries
      .get(bundle_name)
      .and_then(|x| x.staged_version.to_owned());
    Ok(version)
  }

  pub async fn get_entry_status(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Option<BundleManifestEntryItemStatus>> {
    let manifest = self.load().await?.read().await;
    let status = manifest
      .entries
      .get(bundle_name)
      .map(|entry| BundleManifestEntryItemStatus::from(entry, version));
    Ok(status)
  }

  pub async fn get_entry_data(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Option<BundleManifestVersionData>> {
    let manifest = self.load().await?.read().await;
    let data = manifest
      .entries
      .get(bundle_name)
      .and_then(|entry| entry.versions.get(version))
      .cloned();
    Ok(data)
  }

  async fn load(&self) -> crate::Result<&RwLock<BundleManifestData>> {
    let data = self
      .data
      .get_or_try_init(|| async {
        let raw = match util::fs::read_file_with_retry(&self.filepath).await {
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

impl BundleManifest<ReadWrite> {
  pub async fn set_current_version(&self, bundle_name: &str, version: &str) -> crate::Result<()> {
    self
      .set_current_versions(&[(bundle_name.to_owned(), version.to_owned())])
      .await
  }

  pub async fn set_current_versions(&self, items: &[(String, String)]) -> crate::Result<()> {
    let mut manifest = self.load().await?.write().await;

    for (bundle_name, version) in items {
      let entry = manifest
        .entries
        .get(bundle_name)
        .ok_or_else(|| crate::Error::bundle_entry_not_exists(bundle_name, version))?;
      if !entry.versions.contains_key(version) {
        return Err(crate::Error::bundle_entry_not_exists(bundle_name, version));
      }
    }

    for (bundle_name, version) in items {
      let Some(entry) = manifest.entries.get_mut(bundle_name) else {
        continue;
      };

      if entry.previous_version.as_deref() == Some(version.as_str()) {
        entry.previous_version = None;
      } else {
        entry.previous_version = entry.current_version.clone();
      }

      if entry.staged_version.as_deref() == Some(version.as_str()) {
        entry.staged_version = None;
      }

      entry.current_version = Some(version.to_owned());
    }

    Ok(())
  }

  pub async fn insert_stage_entry(
    &self,
    bundle_name: &str,
    version: &str,
    data: BundleManifestVersionData,
  ) -> crate::Result<()> {
    self
      .insert_staged_entries(&[(bundle_name.to_string(), version.to_string(), data)])
      .await
  }

  pub async fn insert_staged_entries(
    &self,
    items: &[(String, String, BundleManifestVersionData)],
  ) -> crate::Result<()> {
    let mut manifest = self.load().await?.write().await;

    for (bundle_name, version, data) in items {
      manifest
        .entries
        .entry(bundle_name.to_string())
        .and_modify(|entry| {
          entry.versions.insert(version.to_string(), data.clone());
          entry.staged_version = Some(version.to_string());
        })
        .or_insert_with(|| BundleManifestEntry {
          versions: HashMap::from([(version.to_string(), data.clone())]),
          current_version: None,
          previous_version: None,
          staged_version: Some(version.to_string()),
        });
    }

    Ok(())
  }

  pub async fn remove_entry(
    &self,
    bundle_name: &str,
    version: &str,
    force: Option<bool>,
  ) -> crate::Result<bool> {
    let force = force.unwrap_or(false);
    let mut data = self.load().await?.write().await;
    if let Some(entry) = data.entries.get_mut(bundle_name) {
      if !force && entry.current_version.as_deref() == Some(version) {
        return Err(crate::Error::bundle_cannot_be_removed(bundle_name, version));
      }
      if entry.previous_version.as_deref() == Some(version) {
        entry.previous_version = None;
      }
      if entry.staged_version.as_deref() == Some(version) {
        entry.staged_version = None;
      }
      return Ok(entry.versions.remove(version).is_some());
    }
    Ok(false)
  }

  pub async fn remove_entries(
    &self,
    items: &[(String, String)],
    force: Option<bool>,
  ) -> crate::Result<Vec<bool>> {
    let force = force.unwrap_or(false);
    let mut data = self.load().await?.write().await;

    if !force {
      for (bundle_name, version) in items.iter() {
        let is_current = data
          .entries
          .get(bundle_name)
          .is_some_and(|entry| entry.current_version.as_deref() == Some(version.as_str()));
        if is_current {
          return Err(crate::Error::bundle_cannot_be_removed(bundle_name, version));
        }
      }
    }

    let mut removed = Vec::with_capacity(items.len());
    for (bundle_name, version) in items {
      let Some(entry) = data.entries.get_mut(bundle_name) else {
        continue;
      };
      let name = bundle_name.to_string();
      if entry.current_version.as_deref() == Some(version) {
        entry.current_version = None;
      }
      if entry.previous_version.as_deref() == Some(version) {
        entry.previous_version = None;
      }
      if entry.staged_version.as_deref() == Some(version) {
        entry.staged_version = None;
      }
      removed.push(entry.versions.remove(version).is_some());
      if entry.versions.is_empty() {
        data.entries.remove(&name);
      }
    }

    Ok(removed)
  }

  pub async fn clear(&self) -> crate::Result<()> {
    let mut data = self.load().await?.write().await;
    *data = BundleManifestData::default();
    Ok(())
  }

  pub async fn save(&self) -> crate::Result<()> {
    let _save = self.save_lock.lock().await;
    let raw = {
      let data = self.load().await?.read().await;
      serde_json::to_vec(&*data)
    }?;

    util::fs::atomic_write_file(&self.filepath, &raw).await?;

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::testing::*;
  use crate::util;
  use std::sync::Arc;

  #[tokio::test]
  async fn list_entries() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadOnly);
    let items = manifest.list_entries().await.unwrap();
    assert_eq!(items.len(), 2);
    let current = items
      .iter()
      .find(|x| x.name == "app" && x.status == BundleManifestEntryItemStatus::Current)
      .unwrap();
    assert_eq!(current.version, "1.0.0");
  }

  #[tokio::test]
  async fn get_entry_metadata() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("builtin/manifest.json"), ReadOnly);
    manifest
      .get_entry_data("app", "1.0.0")
      .await
      .unwrap()
      .unwrap();
    assert!(
      manifest
        .get_entry_data("app", "not_exists")
        .await
        .unwrap()
        .is_none()
    );
  }

  #[tokio::test]
  async fn get_entry_metadata_concurrently() {
    let fixture = Fixtures::bundles();
    let manifest = Arc::new(BundleManifest::new(
      &fixture.get_path("builtin/manifest.json"),
      ReadOnly,
    ));
    let mut handlers = vec![];
    for _ in 1..10 {
      let m = manifest.clone();
      let handle = tokio::spawn(async move { m.get_entry_data("app", "1.0.0").await });
      handlers.push(handle);
    }
    for h in handlers {
      h.await.unwrap().unwrap();
    }
  }

  #[tokio::test]
  async fn get_current_version() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadOnly);
    let version = manifest.get_current_version("app").await.unwrap().unwrap();
    assert_eq!(version, "1.0.0");
  }

  #[tokio::test]
  async fn get_current_version_concurrently() {
    let fixture = Fixtures::bundles();
    let manifest = Arc::new(BundleManifest::new(
      &fixture.get_path("remote/manifest.json"),
      ReadOnly,
    ));
    let mut handlers = vec![];
    for _ in 1..10 {
      let m = manifest.clone();
      let handle = tokio::spawn(async move { m.get_current_version("app").await });
      handlers.push(handle);
    }
    for h in handlers {
      let version = h.await.unwrap().unwrap().unwrap();
      assert_eq!(version, "1.0.0");
    }
  }

  #[tokio::test]
  async fn set_current_version() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadWrite);
    manifest.set_current_version("app", "1.1.0").await.unwrap();
    assert_eq!(
      manifest.get_current_version("app").await.unwrap().unwrap(),
      "1.1.0"
    );
  }

  #[tokio::test]
  async fn set_current_version_entry_not_exists() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadWrite);
    let err = manifest
      .set_current_version("app", "not_exists")
      .await
      .unwrap_err();
    assert_eq!(
      err.to_string(),
      "bundle entry not exists (bundle_name: app, version: not_exists)"
    );
    let err = manifest
      .set_current_version("not_exists", "1.0.0")
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
    let metadata = BundleManifestVersionData {
      etag: None,
      integrity: None,
      signature: None,
      last_modified: None,
    };
    let inserted = manifest
      .insert_stage_entry("app", "1.2.0", metadata.clone())
      .await
      .unwrap();
    assert!(inserted);
    assert_eq!(
      manifest
        .get_entry_data("app", "1.2.0")
        .await
        .unwrap()
        .unwrap(),
      metadata
    );
    // Staging a new version into an existing entry must NOT change the active version.
    assert_eq!(
      manifest.get_current_version("app").await.unwrap().unwrap(),
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
    let metadata = BundleManifestVersionData {
      etag: None,
      integrity: None,
      signature: None,
      last_modified: None,
    };
    let inserted = manifest
      .insert_stage_entry("vite", "1.0.0", metadata.clone())
      .await
      .unwrap();
    assert!(inserted);
    assert_eq!(
      manifest
        .get_entry_data("vite", "1.0.0")
        .await
        .unwrap()
        .unwrap(),
      metadata
    );
    assert!(
      manifest
        .get_current_version("vite")
        .await
        .unwrap()
        .is_none(),
    );
    manifest.set_current_version("vite", "1.0.0").await.unwrap();
    assert_eq!(
      manifest.get_current_version("vite").await.unwrap().unwrap(),
      "1.0.0"
    );
  }

  #[tokio::test]
  async fn remove_entry() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadWrite);
    let removed = manifest.remove_entry("app", "1.1.0", None).await.unwrap();
    assert!(removed);
    assert!(
      manifest
        .get_entry_data("app", "1.1.0")
        .await
        .unwrap()
        .is_none()
    );
  }

  #[tokio::test]
  async fn remove_entry_current_version_cannot_be_removed() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadWrite);
    manifest.set_current_version("app", "1.1.0").await.unwrap();
    let err = manifest
      .remove_entry("app", "1.1.0", None)
      .await
      .unwrap_err();
    assert_eq!(
      err.to_string(),
      "bundle cannot be removed (bundle_name: app, version: 1.1.0)"
    );
  }

  #[tokio::test]
  async fn remove_entry_with_force() {
    let fixture = Fixtures::bundles();
    let manifest = BundleManifest::new(&fixture.get_path("remote/manifest.json"), ReadWrite);
    manifest.set_current_version("app", "1.1.0").await.unwrap();
    let removed = manifest
      .remove_entry("app", "1.1.0", Some(true))
      .await
      .unwrap();
    assert!(removed);
    assert!(
      manifest
        .get_entry_data("app", "1.1.0")
        .await
        .unwrap()
        .is_none()
    );
  }

  #[tokio::test]
  async fn atomic_file_saving() {
    let temp = TempDir::new();
    let filepath = temp.dir().join("remote").join("manifest.json");
    let manifest = BundleManifest::new(&filepath, ReadWrite);
    manifest
      .insert_stage_entry("app", "1.0.0", BundleManifestVersionData::default())
      .await
      .unwrap();
    manifest.set_current_version("app", "1.0.0").await.unwrap();
    manifest.save().await.unwrap();

    let reloaded = BundleManifest::new(&filepath, ReadOnly);
    assert_eq!(
      reloaded.get_current_version("app").await.unwrap().unwrap(),
      "1.0.0"
    );

    let mut dir = tokio::fs::read_dir(filepath.parent().unwrap())
      .await
      .unwrap();
    while let Some(entry) = dir.next_entry().await.unwrap() {
      assert_eq!(
        entry.file_name(),
        "manifest.json",
        "a successful save must not leave its temp file behind"
      );
    }
  }

  #[tokio::test]
  async fn atomic_file_saving_concurrently() {
    let temp = TempDir::new();
    let filepath = temp.dir().join("manifest.json");
    let manifest = Arc::new(BundleManifest::new(&filepath, ReadWrite));
    manifest
      .insert_stage_entry("app", "1.0.0", BundleManifestVersionData::default())
      .await
      .unwrap();
    manifest.save().await.unwrap();

    let mut writers = vec![];
    for i in 0..8 {
      let m = manifest.clone();
      writers.push(tokio::spawn(async move {
        for j in 0..8 {
          m.insert_stage_entry(
            "app",
            &format!("2.{i}.{j}"),
            BundleManifestVersionData::default(),
          )
          .await
          .unwrap();
          m.save().await.unwrap();
        }
      }));
    }

    let reader = {
      let filepath = filepath.clone();
      tokio::spawn(async move {
        for _ in 0..200 {
          let raw = util::fs::read_file_with_retry(&filepath).await.unwrap();
          serde_json::from_slice::<BundleManifestData>(&raw)
            .expect("the manifest on disk must always be a complete document");
          tokio::task::yield_now().await;
        }
      })
    };

    for writer in writers {
      writer.await.unwrap();
    }
    reader.await.unwrap();

    let reloaded = BundleManifest::new(&filepath, ReadOnly);
    assert_eq!(reloaded.list_versions("app").await.unwrap().len(), 65);
  }

  async fn staged_manifest(versions: &[&str]) -> BundleManifest<ReadWrite> {
    let temp = TempDir::new();
    let manifest = BundleManifest::new(&temp.dir().join("manifest.json"), ReadWrite);
    for version in versions {
      manifest
        .insert_stage_entry("app", version, BundleManifestVersionData::default())
        .await
        .unwrap();
    }
    manifest
  }

  async fn entry_item(
    manifest: &BundleManifest<ReadWrite>,
    version: &str,
  ) -> BundleManifestEntryItem {
    manifest
      .list_entries()
      .await
      .unwrap()
      .into_iter()
      .find(|x| x.name == "app" && x.version == version)
      .expect("version must be listed")
  }

  fn removed_versions(items: &[BundleManifestEntryItem]) -> Vec<&str> {
    items.iter().map(|x| x.version.as_str()).collect()
  }

  #[tokio::test]
  async fn remove_entries_rejects_a_current_version_without_force() {
    let manifest = staged_manifest(&["1.0.0", "1.1.0"]).await;
    manifest.set_current_version("app", "1.0.0").await.unwrap();

    let items = vec![
      entry_item(&manifest, "1.1.0").await,
      entry_item(&manifest, "1.0.0").await,
    ];
    let err = manifest.remove_entries(items, None).await.unwrap_err();
    assert!(matches!(
      err,
      crate::Error::BundleCannotBeRemoved { ref bundle_name, ref version }
        if bundle_name == "app" && version == "1.0.0"
    ));
    // The whole batch is rejected before anything is touched, so the version listed
    // before the offending one survives too.
    assert!(manifest.contains_entry("app", "1.1.0").await.unwrap());
  }

  #[tokio::test]
  async fn remove_entries_with_force_removes_the_current_version() {
    let manifest = staged_manifest(&["1.0.0", "1.1.0"]).await;
    manifest.set_current_version("app", "1.0.0").await.unwrap();
    manifest.set_current_version("app", "1.1.0").await.unwrap();

    let items = vec![entry_item(&manifest, "1.1.0").await];
    let removed = manifest.remove_entries(items, Some(true)).await.unwrap();
    assert_eq!(removed_versions(&removed), vec!["1.1.0"]);
    assert!(!manifest.contains_entry("app", "1.1.0").await.unwrap());
    // Neither `current` nor `previous` may keep pointing at the removed version, and the
    // previous version is not silently promoted in its place.
    assert!(manifest.get_current_version("app").await.unwrap().is_none());
    assert_eq!(
      manifest.get_previous_version("app").await.unwrap().unwrap(),
      "1.0.0"
    );
  }

  #[tokio::test]
  async fn remove_entries_returns_only_the_versions_it_removed() {
    let manifest = staged_manifest(&["1.0.0", "1.1.0"]).await;
    let mut items = vec![
      entry_item(&manifest, "1.1.0").await,
      entry_item(&manifest, "1.1.0").await,
    ];
    items.push(BundleManifestEntryItem {
      name: "not_exists".to_string(),
      version: "1.0.0".to_string(),
      status: BundleManifestEntryItemStatus::Orphan,
      data: BundleManifestVersionData::default(),
    });
    // Removed once: the duplicate is already gone, and the unknown bundle never existed.
    let removed = manifest.remove_entries(items, None).await.unwrap();
    assert_eq!(removed_versions(&removed), vec!["1.1.0"]);
    assert!(manifest.contains_entry("app", "1.0.0").await.unwrap());
  }

  #[tokio::test]
  async fn remove_entries_drops_an_entry_left_without_versions() {
    let manifest = staged_manifest(&["1.0.0"]).await;
    let items = vec![entry_item(&manifest, "1.0.0").await];
    let removed = manifest.remove_entries(items, None).await.unwrap();
    assert_eq!(removed_versions(&removed), vec!["1.0.0"]);
    assert!(manifest.list_entries().await.unwrap().is_empty());
    assert!(manifest.list_versions("app").await.unwrap().is_empty());
  }

  #[tokio::test]
  async fn remove_entry_clears_previous_pointer() {
    let manifest = staged_manifest(&["1.0.0", "1.1.0"]).await;
    manifest.set_current_version("app", "1.0.0").await.unwrap();
    manifest.set_current_version("app", "1.1.0").await.unwrap();
    assert_eq!(
      manifest.get_previous_version("app").await.unwrap().unwrap(),
      "1.0.0"
    );
    // Removing the previous version must not leave `previous` pointing at it.
    assert!(manifest.remove_entry("app", "1.0.0", None).await.unwrap());
    assert!(
      manifest
        .get_previous_version("app")
        .await
        .unwrap()
        .is_none()
    );
  }
}
