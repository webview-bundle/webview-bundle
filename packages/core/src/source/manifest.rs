use crate::source::{
  ManifestBundleItem, ManifestBundleItemStatus, ManifestData, ManifestPruneResult,
  ManifestRemoveData, ManifestRemoveResult, ManifestRemoveResultKind,
  ManifestSetCurrentVersionResult, ManifestStageData, ManifestStageResult, ManifestStageResultKind,
  ManifestVersionData,
};
use crate::util;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use tokio::sync::OnceCell;

pub trait ManifestMode: Send + Sync + 'static {}

#[derive(Debug)]
pub struct ReadOnly;
impl ManifestMode for ReadOnly {}

#[derive(Debug)]
pub struct ReadWrite;
impl ManifestMode for ReadWrite {}

#[derive(Debug)]
pub struct Manifest<Mode: ManifestMode> {
  _mode: std::marker::PhantomData<Mode>,
  filepath: PathBuf,
  data: OnceCell<ManifestData>,
}

impl<Mode> Manifest<Mode>
where
  Mode: ManifestMode,
{
  pub fn new(filepath: &Path, _mode: Mode) -> Self {
    Self {
      _mode: std::marker::PhantomData,
      filepath: filepath.to_path_buf(),
      data: Default::default(),
    }
  }

  pub fn filepath(&self) -> &Path {
    &self.filepath
  }

  pub async fn list_items(&self) -> crate::Result<Vec<ManifestBundleItem>> {
    let manifest = self.load().await?;
    let mut items = vec![];
    for (bundle_name, entry) in manifest.bundles.iter() {
      for (version, data) in entry.versions.iter() {
        let item = ManifestBundleItem {
          name: bundle_name.to_string(),
          version: version.to_string(),
          status: ManifestBundleItemStatus::from(entry, version),
          data: data.clone(),
        };
        items.push(item);
      }
    }
    Ok(items)
  }

  pub async fn contains(&self, bundle_name: &str, version: &str) -> crate::Result<bool> {
    let manifest = self.load().await?;
    if let Some(entry) = manifest.bundles.get(bundle_name) {
      return Ok(entry.versions.contains_key(version));
    }
    Ok(false)
  }

  pub async fn get_current_version(&self, bundle_name: &str) -> crate::Result<Option<String>> {
    let manifest = self.load().await?;
    let version = manifest
      .bundles
      .get(bundle_name)
      .and_then(|x| x.current_version.to_owned());
    Ok(version)
  }

  pub async fn get_previous_version(&self, bundle_name: &str) -> crate::Result<Option<String>> {
    let manifest = self.load().await?;
    let version = manifest
      .bundles
      .get(bundle_name)
      .and_then(|x| x.previous_version.to_owned());
    Ok(version)
  }

  pub async fn get_staged_version(&self, bundle_name: &str) -> crate::Result<Option<String>> {
    let manifest = self.load().await?;
    let version = manifest
      .bundles
      .get(bundle_name)
      .and_then(|x| x.staged_version.to_owned());
    Ok(version)
  }

  pub async fn get_status(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Option<ManifestBundleItemStatus>> {
    let manifest = self.load().await?;
    let status = manifest
      .bundles
      .get(bundle_name)
      .map(|entry| ManifestBundleItemStatus::from(entry, version));
    Ok(status)
  }

  pub async fn get_version_data(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<Option<ManifestVersionData>> {
    let manifest = self.load().await?;
    let data = manifest
      .bundles
      .get(bundle_name)
      .and_then(|entry| entry.versions.get(version))
      .cloned();
    Ok(data)
  }

  async fn load(&self) -> crate::Result<&ManifestData> {
    let data = self
      .data
      .get_or_try_init(|| async {
        let raw = match util::fs::read_file_with_retry(&self.filepath).await {
          Ok(raw) => raw,
          Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok::<ManifestData, crate::Error>(Default::default());
          }
          Err(e) => return Err(e.into()),
        };
        let data: ManifestData = serde_json::from_slice(&raw)?;
        Ok::<ManifestData, crate::Error>(data)
      })
      .await?;
    Ok(data)
  }
}

impl Manifest<ReadWrite> {
  pub async fn set_current_version(
    &mut self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<ManifestSetCurrentVersionResult> {
    let result = self
      .set_current_version_many([(bundle_name.to_owned(), version.to_owned())])
      .await?
      .first()
      .unwrap()
      .clone();
    Ok(result)
  }

  pub async fn set_current_version_many(
    &mut self,
    items: impl Into<HashMap<String, String>>,
  ) -> crate::Result<Vec<ManifestSetCurrentVersionResult>> {
    let items = items.into();
    if items.is_empty() {
      return Ok(vec![]);
    }

    let mut updated = self.load().await?.clone();
    let mut results = Vec::with_capacity(items.len());
    let mut changed = false;

    for (name, version) in items.into_iter() {
      let Some(bundle) = updated.bundles.get_mut(&name) else {
        results.push(ManifestSetCurrentVersionResult::not_exists(name, version));
        continue;
      };

      if !bundle.versions.contains_key(&version) {
        results.push(ManifestSetCurrentVersionResult::version_not_exists(
          name, version,
        ));
        continue;
      }
      if bundle.current_version.as_deref() == Some(&version) {
        results.push(ManifestSetCurrentVersionResult::settled(name, version));
        continue;
      }

      let previous_version = bundle.current_version.clone();

      if bundle.previous_version.as_deref() == Some(&version) {
        bundle.previous_version = None;
      } else {
        bundle.previous_version = previous_version;
      }
      if bundle.staged_version.as_deref() == Some(&version) {
        bundle.staged_version = None;
      }
      bundle.current_version = Some(version.to_owned());
      changed = true;
      results.push(ManifestSetCurrentVersionResult::settled(name, version));
    }

    if changed {
      self.save(updated).await?;
    }

    Ok(results)
  }

  pub async fn stage(
    &mut self,
    bundle_name: &str,
    data: ManifestStageData,
  ) -> crate::Result<ManifestStageResult> {
    let result = self
      .stage_many([(bundle_name.to_string(), data)])
      .await?
      .first()
      .unwrap()
      .clone();
    Ok(result)
  }

  pub async fn stage_many(
    &mut self,
    items: impl Into<HashMap<String, ManifestStageData>>,
  ) -> crate::Result<Vec<ManifestStageResult>> {
    let items = items.into();
    if items.is_empty() {
      return Ok(vec![]);
    }

    let mut updated = self.load().await?.clone();
    let mut results = Vec::with_capacity(items.len());

    for (name, item) in items.into_iter() {
      let version = item.version;
      let data = item.data.unwrap_or_default();
      let bundle = updated.bundles.entry(name.to_owned()).or_default();

      if bundle.current_version.as_deref() == Some(version.as_str()) {
        results.push(ManifestStageResult::in_use(name, version));
        continue;
      }

      bundle.versions.insert(version.to_owned(), data);
      bundle.staged_version = Some(version.to_owned());
      results.push(ManifestStageResult::staged(name, version));
    }

    if results
      .iter()
      .any(|x| x.kind == ManifestStageResultKind::Staged)
    {
      self.save(updated).await?;
    }

    Ok(results)
  }

  pub async fn remove(
    &mut self,
    bundle_name: &str,
    version: &str,
    force: Option<bool>,
  ) -> crate::Result<ManifestRemoveResult> {
    let result = self
      .remove_many([(
        bundle_name.to_string(),
        ManifestRemoveData {
          versions: vec![version.to_string()],
          force,
        },
      )])
      .await?
      .first()
      .unwrap()
      .clone();
    Ok(result)
  }

  pub async fn remove_many(
    &mut self,
    items: impl Into<HashMap<String, ManifestRemoveData>>,
  ) -> crate::Result<Vec<ManifestRemoveResult>> {
    let items = items.into();
    if items.is_empty() {
      return Ok(vec![]);
    }

    let mut updated = self.load().await?.clone();
    let mut results = Vec::with_capacity(items.len());

    for (name, data) in items.into_iter() {
      let force = data.force.unwrap_or(false);
      let mut versions = data.versions;
      versions.sort();
      versions.dedup();

      for version in versions {
        let Some(bundle) = updated.bundles.get_mut(&name) else {
          results.push(ManifestRemoveResult::not_exists(&name, &version));
          continue;
        };

        if bundle.current_version.as_deref() == Some(&version) {
          if force {
            bundle.current_version = None;
          } else {
            results.push(ManifestRemoveResult::in_use(&name, &version));
            continue;
          }
        }
        if bundle.previous_version.as_deref() == Some(&version) {
          bundle.previous_version = None;
        }
        if bundle.staged_version.as_deref() == Some(&version) {
          bundle.staged_version = None;
        }

        let result = match bundle.versions.remove(&version).is_some() {
          true => ManifestRemoveResult::removed(&name, &version),
          false => ManifestRemoveResult::version_not_exists(&name, &version),
        };
        results.push(result);
      }

      if let Some(bundle) = updated.bundles.get_mut(&name)
        && bundle.versions.is_empty()
      {
        updated.bundles.remove(&name);
      }
    }

    if results
      .iter()
      .any(|x| x.kind == ManifestRemoveResultKind::Removed)
    {
      self.save(updated).await?;
    }

    Ok(results)
  }

  pub async fn prune(&mut self, bundle_name: &str) -> crate::Result<ManifestPruneResult> {
    let result = self
      .prune_many(&[bundle_name])
      .await?
      .first()
      .unwrap()
      .clone();
    Ok(result)
  }

  /// Drops every version which is neither current, previous nor staged, for each of
  /// `bundle_names`, and reports what was dropped for each of them.
  pub async fn prune_many<N>(
    &mut self,
    bundle_names: &[N],
  ) -> crate::Result<Vec<ManifestPruneResult>>
  where
    N: AsRef<str>,
  {
    let mut bundle_names = bundle_names
      .iter()
      .map(|x| x.as_ref().to_string())
      .collect::<Vec<_>>();

    bundle_names.sort();
    bundle_names.dedup();

    if bundle_names.is_empty() {
      return Ok(vec![]);
    }

    let mut updated = self.load().await?.clone();
    let mut results = Vec::with_capacity(bundle_names.len());
    let mut pruned = false;

    for name in bundle_names.into_iter() {
      let Some(bundle) = updated.bundles.get_mut(&name) else {
        results.push(ManifestPruneResult {
          name,
          pruned_versions: vec![],
        });
        continue;
      };

      let mut pruned_versions = bundle
        .versions
        .iter()
        .filter_map(
          |(version, _)| match ManifestBundleItemStatus::from(bundle, version) {
            ManifestBundleItemStatus::Orphan => Some(version.to_owned()),
            _ => None,
          },
        )
        .collect::<Vec<_>>();
      pruned_versions.sort();

      if !pruned_versions.is_empty() {
        pruned = true;
      }

      for prune_version in pruned_versions.iter() {
        bundle.versions.remove(prune_version);
      }

      results.push(ManifestPruneResult {
        name,
        pruned_versions,
      })
    }

    if pruned {
      self.save(updated).await?;
    }

    Ok(results)
  }

  pub async fn clear(&mut self) -> crate::Result<()> {
    self.save(ManifestData::default()).await
  }

  async fn save(&mut self, updated: ManifestData) -> crate::Result<()> {
    let raw = serde_json::to_vec(&updated)?;
    util::fs::atomic_write_file(&self.filepath, &raw).await?;
    self.data = OnceCell::new_with(Some(updated));
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::source::{ManifestSetCurrentVersionResultKind, ManifestVersion};
  use crate::testing::TempDir;
  use std::sync::Arc;
  use tokio::sync::RwLock;

  fn filepath(temp: &TempDir) -> PathBuf {
    temp.dir().join("manifest.json")
  }

  fn manifest(temp: &TempDir) -> Manifest<ReadWrite> {
    Manifest::new(&filepath(temp), ReadWrite)
  }

  /// The manifest as it is on disk, so a test can tell what a write actually persisted.
  fn reopened(temp: &TempDir) -> Manifest<ReadOnly> {
    Manifest::new(&filepath(temp), ReadOnly)
  }

  fn stage_data(version: &str) -> ManifestStageData {
    ManifestStageData {
      version: version.to_owned(),
      data: None,
    }
  }

  fn remove_data(versions: &[&str], force: Option<bool>) -> ManifestRemoveData {
    ManifestRemoveData {
      versions: versions.iter().map(|x| (*x).to_owned()).collect(),
      force,
    }
  }

  /// A manifest holding `versions` of `app`, the last one left staged.
  async fn staged_manifest(temp: &TempDir, versions: &[&str]) -> Manifest<ReadWrite> {
    let mut manifest = manifest(temp);
    for version in versions {
      manifest.stage("app", stage_data(version)).await.unwrap();
    }
    manifest
  }

  async fn versions_by_status<Mode: ManifestMode>(
    manifest: &Manifest<Mode>,
  ) -> Vec<(String, ManifestBundleItemStatus)> {
    let mut items = manifest
      .list_items()
      .await
      .unwrap()
      .into_iter()
      .map(|x| (x.version, x.status))
      .collect::<Vec<_>>();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    items
  }

  #[tokio::test]
  async fn list_items_reports_the_status_of_every_version() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0", "1.1.0", "1.2.0"]).await;
    manifest.set_current_version("app", "1.0.0").await.unwrap();
    manifest.set_current_version("app", "1.1.0").await.unwrap();

    assert_eq!(
      versions_by_status(&manifest).await,
      vec![
        ("1.0.0".to_owned(), ManifestBundleItemStatus::Previous),
        ("1.1.0".to_owned(), ManifestBundleItemStatus::Current),
        ("1.2.0".to_owned(), ManifestBundleItemStatus::Staged),
      ]
    );
  }

  #[tokio::test]
  async fn get_version_data_returns_the_data_it_was_staged_with() {
    let temp = TempDir::new();
    let mut manifest = manifest(&temp);
    let data = ManifestVersionData {
      integrity: Some("sha256:abc".to_owned()),
      metadata: Some(HashMap::from([("channel".to_owned(), "stable".to_owned())])),
    };

    manifest
      .stage(
        "app",
        ManifestStageData {
          version: "1.0.0".to_owned(),
          data: Some(data.clone()),
        },
      )
      .await
      .unwrap();

    assert_eq!(
      manifest.get_version_data("app", "1.0.0").await.unwrap(),
      Some(data.clone())
    );
    assert!(
      manifest
        .get_version_data("app", "9.9.9")
        .await
        .unwrap()
        .is_none()
    );
    assert_eq!(
      reopened(&temp)
        .get_version_data("app", "1.0.0")
        .await
        .unwrap(),
      Some(data)
    );
  }

  #[tokio::test]
  async fn reads_are_shared_between_concurrent_callers() {
    let temp = TempDir::new();
    staged_manifest(&temp, &["1.0.0"]).await;
    let manifest = Arc::new(reopened(&temp));

    let mut handles = vec![];
    for _ in 0..10 {
      let manifest = manifest.clone();
      handles.push(tokio::spawn(async move {
        manifest.get_staged_version("app").await
      }));
    }
    for handle in handles {
      assert_eq!(handle.await.unwrap().unwrap().as_deref(), Some("1.0.0"));
    }
  }

  #[tokio::test]
  async fn set_current_version_activates_a_staged_version() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0"]).await;

    let result = manifest.set_current_version("app", "1.0.0").await.unwrap();

    assert_eq!(
      result,
      ManifestSetCurrentVersionResult::settled("app", "1.0.0")
    );
    assert_eq!(
      manifest
        .get_current_version("app")
        .await
        .unwrap()
        .as_deref(),
      Some("1.0.0")
    );
    assert!(manifest.get_staged_version("app").await.unwrap().is_none());
  }

  #[tokio::test]
  async fn a_write_is_served_without_reloading_the_file() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0", "1.1.0"]).await;

    manifest.set_current_version("app", "1.0.0").await.unwrap();
    manifest.stage("app", stage_data("1.1.0")).await.unwrap();
    manifest.remove("app", "1.1.0", None).await.unwrap();

    assert_eq!(
      manifest
        .get_current_version("app")
        .await
        .unwrap()
        .as_deref(),
      Some("1.0.0")
    );
    assert!(manifest.get_staged_version("app").await.unwrap().is_none());
    assert!(!manifest.contains("app", "1.1.0").await.unwrap());
    assert_eq!(
      versions_by_status(&reopened(&temp)).await,
      versions_by_status(&manifest).await,
      "what is served must be what was written to disk"
    );
  }

  #[tokio::test]
  async fn set_current_version_records_the_version_it_replaced() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0", "1.1.0"]).await;

    manifest.set_current_version("app", "1.0.0").await.unwrap();
    assert!(
      manifest
        .get_previous_version("app")
        .await
        .unwrap()
        .is_none()
    );

    manifest.set_current_version("app", "1.1.0").await.unwrap();
    assert_eq!(
      manifest
        .get_previous_version("app")
        .await
        .unwrap()
        .as_deref(),
      Some("1.0.0")
    );

    // Going back to the previous version leaves nothing to go back to.
    manifest.set_current_version("app", "1.0.0").await.unwrap();
    assert_eq!(
      manifest
        .get_current_version("app")
        .await
        .unwrap()
        .as_deref(),
      Some("1.0.0")
    );
    assert!(
      manifest
        .get_previous_version("app")
        .await
        .unwrap()
        .is_none()
    );
  }

  #[tokio::test]
  async fn setting_the_current_version_again_keeps_the_previous_one() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0", "1.1.0"]).await;
    manifest.set_current_version("app", "1.0.0").await.unwrap();
    manifest.set_current_version("app", "1.1.0").await.unwrap();

    let result = manifest.set_current_version("app", "1.1.0").await.unwrap();

    assert_eq!(result.kind, ManifestSetCurrentVersionResultKind::Settled);
    assert_eq!(
      manifest
        .get_previous_version("app")
        .await
        .unwrap()
        .as_deref(),
      Some("1.0.0")
    );
  }

  #[tokio::test]
  async fn set_current_version_reports_what_does_not_exist() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0"]).await;

    assert_eq!(
      manifest.set_current_version("app", "9.9.9").await.unwrap(),
      ManifestSetCurrentVersionResult::version_not_exists("app", "9.9.9")
    );
    assert_eq!(
      manifest.set_current_version("docs", "1.0.0").await.unwrap(),
      ManifestSetCurrentVersionResult::not_exists("docs", "1.0.0")
    );
    assert!(manifest.get_current_version("app").await.unwrap().is_none());
  }

  #[tokio::test]
  async fn a_version_that_settled_nothing_does_not_write_the_manifest() {
    let temp = TempDir::new();
    let mut manifest = manifest(&temp);

    let results = manifest
      .set_current_version_many([("app".to_owned(), "1.0.0".to_owned())])
      .await
      .unwrap();

    assert_eq!(
      results,
      vec![ManifestSetCurrentVersionResult::not_exists("app", "1.0.0")]
    );
    assert!(!filepath(&temp).exists());
  }

  #[tokio::test]
  async fn a_failed_save_leaves_the_manifest_as_it_was() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0"]).await;
    // A directory where the manifest file should be, so the write can never land.
    std::fs::remove_file(filepath(&temp)).unwrap();
    std::fs::create_dir(filepath(&temp)).unwrap();

    manifest
      .set_current_version("app", "1.0.0")
      .await
      .unwrap_err();

    assert!(manifest.get_current_version("app").await.unwrap().is_none());
    assert_eq!(
      manifest.get_staged_version("app").await.unwrap().as_deref(),
      Some("1.0.0")
    );
  }

  #[tokio::test]
  async fn stage_reports_the_version_it_staged() {
    let temp = TempDir::new();
    let mut manifest = manifest(&temp);

    let result = manifest.stage("app", stage_data("1.0.0")).await.unwrap();

    assert_eq!(result, ManifestStageResult::staged("app", "1.0.0"));
    assert_eq!(
      manifest.get_staged_version("app").await.unwrap().as_deref(),
      Some("1.0.0")
    );
    assert!(manifest.get_current_version("app").await.unwrap().is_none());
    assert_eq!(
      reopened(&temp)
        .get_staged_version("app")
        .await
        .unwrap()
        .as_deref(),
      Some("1.0.0")
    );
  }

  #[tokio::test]
  async fn stage_many_stages_every_item() {
    let temp = TempDir::new();
    let mut manifest = manifest(&temp);

    let mut results = manifest
      .stage_many([
        ("app".to_owned(), stage_data("1.0.0")),
        ("docs".to_owned(), stage_data("2.0.0")),
      ])
      .await
      .unwrap();
    results.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(
      results,
      vec![
        ManifestStageResult::staged("app", "1.0.0"),
        ManifestStageResult::staged("docs", "2.0.0"),
      ]
    );
    let reopened = reopened(&temp);
    assert_eq!(
      reopened.get_staged_version("app").await.unwrap().as_deref(),
      Some("1.0.0")
    );
    assert_eq!(
      reopened
        .get_staged_version("docs")
        .await
        .unwrap()
        .as_deref(),
      Some("2.0.0")
    );
  }

  #[tokio::test]
  async fn stage_replaces_the_version_which_was_staged_before() {
    let temp = TempDir::new();
    let manifest = staged_manifest(&temp, &["1.0.0", "1.1.0"]).await;

    assert_eq!(
      manifest.get_staged_version("app").await.unwrap().as_deref(),
      Some("1.1.0")
    );
    assert!(manifest.contains("app", "1.0.0").await.unwrap());
  }

  #[tokio::test]
  async fn stage_refuses_the_current_version() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0"]).await;
    manifest.set_current_version("app", "1.0.0").await.unwrap();

    let result = manifest.stage("app", stage_data("1.0.0")).await.unwrap();

    assert_eq!(result, ManifestStageResult::in_use("app", "1.0.0"));
    assert!(manifest.get_staged_version("app").await.unwrap().is_none());
  }

  #[tokio::test]
  async fn remove_drops_the_version_and_the_pointers_to_it() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0", "1.1.0"]).await;
    manifest.set_current_version("app", "1.0.0").await.unwrap();
    manifest.set_current_version("app", "1.1.0").await.unwrap();

    let result = manifest.remove("app", "1.0.0", None).await.unwrap();

    assert_eq!(result, ManifestRemoveResult::removed("app", "1.0.0"));
    assert!(!manifest.contains("app", "1.0.0").await.unwrap());
    assert!(
      manifest
        .get_previous_version("app")
        .await
        .unwrap()
        .is_none()
    );
  }

  #[tokio::test]
  async fn remove_keeps_the_current_version_unless_it_is_forced() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0", "1.1.0"]).await;
    manifest.set_current_version("app", "1.0.0").await.unwrap();
    manifest.set_current_version("app", "1.1.0").await.unwrap();

    assert_eq!(
      manifest.remove("app", "1.1.0", None).await.unwrap(),
      ManifestRemoveResult::in_use("app", "1.1.0")
    );
    assert!(manifest.contains("app", "1.1.0").await.unwrap());

    assert_eq!(
      manifest.remove("app", "1.1.0", Some(true)).await.unwrap(),
      ManifestRemoveResult::removed("app", "1.1.0")
    );
    // Neither `current` nor `previous` may keep pointing at the removed version, and the
    // previous version is not silently promoted in its place.
    assert!(manifest.get_current_version("app").await.unwrap().is_none());
    assert_eq!(
      manifest
        .get_previous_version("app")
        .await
        .unwrap()
        .as_deref(),
      Some("1.0.0")
    );
  }

  #[tokio::test]
  async fn remove_many_reports_every_version_it_was_given() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0", "1.1.0"]).await;

    let mut results = manifest
      .remove_many([
        (
          "app".to_owned(),
          remove_data(&["1.1.0", "1.1.0", "9.9.9"], None),
        ),
        ("docs".to_owned(), remove_data(&["1.0.0"], None)),
      ])
      .await
      .unwrap();
    results.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));

    assert_eq!(
      results,
      vec![
        ManifestRemoveResult::removed("app", "1.1.0"),
        ManifestRemoveResult::version_not_exists("app", "9.9.9"),
        ManifestRemoveResult::not_exists("docs", "1.0.0"),
      ]
    );
    assert!(manifest.contains("app", "1.0.0").await.unwrap());
  }

  #[tokio::test]
  async fn remove_drops_a_bundle_left_without_versions() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0"]).await;

    manifest.remove("app", "1.0.0", None).await.unwrap();

    assert!(manifest.list_items().await.unwrap().is_empty());
    assert!(reopened(&temp).list_items().await.unwrap().is_empty());
  }

  #[tokio::test]
  async fn prune_drops_the_orphans_of_every_bundle_it_was_given() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0", "1.1.0", "1.2.0", "1.3.0"]).await;
    manifest.set_current_version("app", "1.0.0").await.unwrap();
    manifest.set_current_version("app", "1.1.0").await.unwrap();

    let results = manifest.prune_many(&["app", "docs"]).await.unwrap();

    assert_eq!(
      results,
      vec![
        ManifestPruneResult {
          name: "app".to_owned(),
          pruned_versions: vec!["1.2.0".to_owned()],
        },
        ManifestPruneResult {
          name: "docs".to_owned(),
          pruned_versions: vec![],
        },
      ]
    );
    assert_eq!(
      versions_by_status(&reopened(&temp)).await,
      vec![
        ("1.0.0".to_owned(), ManifestBundleItemStatus::Previous),
        ("1.1.0".to_owned(), ManifestBundleItemStatus::Current),
        ("1.3.0".to_owned(), ManifestBundleItemStatus::Staged),
      ]
    );
  }

  #[tokio::test]
  async fn clear_empties_the_manifest() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0"]).await;

    manifest.clear().await.unwrap();

    assert!(manifest.list_items().await.unwrap().is_empty());
    assert!(reopened(&temp).list_items().await.unwrap().is_empty());
  }

  #[tokio::test]
  async fn a_successful_write_leaves_no_temp_file_behind() {
    let temp = TempDir::new();
    let mut manifest = staged_manifest(&temp, &["1.0.0"]).await;
    manifest.set_current_version("app", "1.0.0").await.unwrap();

    let mut dir = tokio::fs::read_dir(temp.dir()).await.unwrap();
    while let Some(entry) = dir.next_entry().await.unwrap() {
      assert_eq!(entry.file_name(), "manifest.json");
    }
  }

  #[tokio::test]
  async fn concurrent_writes_keep_the_file_a_complete_document() {
    let temp = TempDir::new();
    let manifest = Arc::new(RwLock::new(manifest(&temp)));
    manifest
      .write()
      .await
      .stage("app", stage_data("1.0.0"))
      .await
      .unwrap();

    let mut writers = vec![];
    for i in 0..8 {
      let manifest = manifest.clone();
      writers.push(tokio::spawn(async move {
        for j in 0..8 {
          manifest
            .write()
            .await
            .stage("app", stage_data(&format!("2.{i}.{j}")))
            .await
            .unwrap();
        }
      }));
    }

    let reader = {
      let filepath = filepath(&temp);
      tokio::spawn(async move {
        for _ in 0..200 {
          let raw = util::fs::read_file_with_retry(&filepath).await.unwrap();
          let data = serde_json::from_slice::<ManifestData>(&raw)
            .expect("the manifest on disk must always be a complete document");
          assert_eq!(data.manifest_version, ManifestVersion::V1);
          tokio::task::yield_now().await;
        }
      })
    };

    for writer in writers {
      writer.await.unwrap();
    }
    reader.await.unwrap();

    assert_eq!(manifest.read().await.list_items().await.unwrap().len(), 65);
    assert_eq!(reopened(&temp).list_items().await.unwrap().len(), 65);
  }
}
