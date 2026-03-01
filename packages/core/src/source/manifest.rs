use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use tokio::sync::{OnceCell, RwLock};

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BundleManifestVersion {
  #[default]
  V1 = 1,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BundleManifestMetadata {
  pub etag: Option<String>,
  pub integrity: Option<String>,
  pub signature: Option<String>,
  pub last_modified: Option<String>,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BundleManifestEntry {
  pub versions: HashMap<String, BundleManifestMetadata>,
  pub current_version: String,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct BundleManifestData {
  pub manifest_version: BundleManifestVersion,
  pub entries: HashMap<String, BundleManifestEntry>,
}

#[derive(Debug, Clone)]
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
    }
  }

  pub async fn list_entries(&self) -> crate::Result<Vec<ListBundleManifestItem>> {
    let data = self.load().await?.read().await;
    let mut items = vec![];
    for (bundle_name, entry) in data.entries.iter() {
      let current_version = entry.current_version.to_string();
      for (version, metadata) in entry.versions.iter() {
        let item = ListBundleManifestItem {
          name: bundle_name.to_string(),
          version: version.to_string(),
          current: version == &current_version,
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
      .map(|x| x.current_version.to_string());
    Ok(version)
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
        if !tokio::fs::try_exists(&self.filepath).await? {
          return Ok::<RwLock<BundleManifestData>, crate::Error>(Default::default());
        }
        let raw = tokio::fs::read(&self.filepath).await?;
        let data: BundleManifestData = serde_json::from_slice(&raw)?;
        Ok::<RwLock<BundleManifestData>, crate::Error>(RwLock::new(data))
      })
      .await?;
    Ok(data)
  }
}

impl BundleManifest<ReadWrite> {
  pub async fn update_current_version(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<()> {
    if !self.contains_entry(bundle_name, version).await? {
      return Err(crate::Error::bundle_entry_not_exists(bundle_name, version));
    }
    let mut data = self.load().await?.write().await;
    data
      .entries
      .entry(bundle_name.to_string())
      .and_modify(|entry| {
        entry.current_version = version.to_string();
      });
    Ok(())
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
        current_version: version.to_string(),
      });
    Ok(inserted)
  }

  pub async fn remove_entry(&self, bundle_name: &str, version: &str) -> crate::Result<bool> {
    let mut data = self.load().await?.write().await;
    if let Some(entry) = data.entries.get_mut(bundle_name) {
      if entry.current_version == version {
        return Err(crate::Error::bundle_cannot_be_removed(
          bundle_name,
          version,
          "current version of bundle cannot be removed",
        ));
      }
      return Ok(entry.versions.remove(version).is_some());
    }
    Ok(false)
  }

  pub async fn save(&self) -> crate::Result<()> {
    let raw = {
      let data = self.load().await?.read().await;
      serde_json::to_vec(&*data)
    }?;
    if let Some(dir) = self.filepath.parent() {
      tokio::fs::create_dir_all(dir).await?;
    }
    tokio::fs::write(&self.filepath, raw).await?;
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
}
