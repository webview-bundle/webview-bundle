use crate::WebviewBundleExtra;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Runtime, command};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersionData {
  pub integrity: Option<String>,
  pub metadata: Option<HashMap<String, String>>,
}

impl From<wvb::source::ManifestVersionData> for ManifestVersionData {
  fn from(value: wvb::source::ManifestVersionData) -> Self {
    Self {
      integrity: value.integrity.clone(),
      metadata: value.metadata.clone(),
    }
  }
}

impl From<ManifestVersionData> for wvb::source::ManifestVersionData {
  fn from(value: ManifestVersionData) -> Self {
    Self {
      integrity: value.integrity,
      metadata: value.metadata,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestBundleItemStatus {
  Current,
  Previous,
  Staged,
  Orphan,
}

impl From<wvb::source::ManifestBundleItemStatus> for ManifestBundleItemStatus {
  fn from(value: wvb::source::ManifestBundleItemStatus) -> Self {
    match value {
      wvb::source::ManifestBundleItemStatus::Current => Self::Current,
      wvb::source::ManifestBundleItemStatus::Previous => Self::Previous,
      wvb::source::ManifestBundleItemStatus::Staged => Self::Staged,
      wvb::source::ManifestBundleItemStatus::Orphan => Self::Orphan,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestBundleItem {
  pub name: String,
  pub version: String,
  pub status: ManifestBundleItemStatus,
  pub data: ManifestVersionData,
}

impl From<wvb::source::ManifestBundleItem> for ManifestBundleItem {
  fn from(value: wvb::source::ManifestBundleItem) -> Self {
    Self {
      name: value.name,
      version: value.version,
      status: value.status.into(),
      data: value.data.into(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
  Builtin,
  Remote,
}

impl From<wvb::source::SourceKind> for SourceKind {
  fn from(value: wvb::source::SourceKind) -> Self {
    match value {
      wvb::source::SourceKind::Builtin => SourceKind::Builtin,
      wvb::source::SourceKind::Remote => SourceKind::Remote,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestListItem {
  pub source: SourceKind,
  #[serde(flatten)]
  pub item: ManifestBundleItem,
}

impl From<wvb::source::SourceListItem> for ManifestListItem {
  fn from(value: wvb::source::SourceListItem) -> Self {
    Self {
      source: value.source.into(),
      item: value.item.into(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleSourceVersion {
  pub source: SourceKind,
  pub version: String,
}

impl From<wvb::source::BundleSourceVersion> for BundleSourceVersion {
  fn from(value: wvb::source::BundleSourceVersion) -> Self {
    Self {
      source: value.source.into(),
      version: value.version,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestSetCurrentVersionResultKind {
  Settled,
  NotExists,
  VersionNotExists,
}

impl From<wvb::source::ManifestSetCurrentVersionResultKind>
  for ManifestSetCurrentVersionResultKind
{
  fn from(value: wvb::source::ManifestSetCurrentVersionResultKind) -> Self {
    match value {
      wvb::source::ManifestSetCurrentVersionResultKind::Settled => Self::Settled,
      wvb::source::ManifestSetCurrentVersionResultKind::NotExists => Self::NotExists,
      wvb::source::ManifestSetCurrentVersionResultKind::VersionNotExists => Self::VersionNotExists,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSetCurrentVersionResult {
  pub name: String,
  pub version: String,
  pub kind: ManifestSetCurrentVersionResultKind,
}

impl From<wvb::source::ManifestSetCurrentVersionResult> for ManifestSetCurrentVersionResult {
  fn from(value: wvb::source::ManifestSetCurrentVersionResult) -> Self {
    Self {
      name: value.name,
      version: value.version,
      kind: value.kind.into(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestStageData {
  pub version: String,
  pub data: Option<ManifestVersionData>,
}

impl From<ManifestStageData> for wvb::source::ManifestStageData {
  fn from(value: ManifestStageData) -> Self {
    Self {
      version: value.version,
      data: value.data.map(Into::into),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestStageResultKind {
  Staged,
  InUse,
}

impl From<wvb::source::ManifestStageResultKind> for ManifestStageResultKind {
  fn from(value: wvb::source::ManifestStageResultKind) -> Self {
    match value {
      wvb::source::ManifestStageResultKind::Staged => Self::Staged,
      wvb::source::ManifestStageResultKind::InUse => Self::InUse,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestStageResult {
  pub name: String,
  pub version: String,
  pub kind: ManifestStageResultKind,
}

impl From<wvb::source::ManifestStageResult> for ManifestStageResult {
  fn from(value: wvb::source::ManifestStageResult) -> Self {
    Self {
      name: value.name,
      version: value.version,
      kind: value.kind.into(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRemoveData {
  pub versions: Vec<String>,
  pub force: Option<bool>,
}

impl From<ManifestRemoveData> for wvb::source::ManifestRemoveData {
  fn from(value: ManifestRemoveData) -> Self {
    Self {
      versions: value.versions,
      force: value.force,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestRemoveResultKind {
  Removed,
  NotExists,
  VersionNotExists,
  InUse,
}

impl From<wvb::source::ManifestRemoveResultKind> for ManifestRemoveResultKind {
  fn from(value: wvb::source::ManifestRemoveResultKind) -> Self {
    match value {
      wvb::source::ManifestRemoveResultKind::Removed => Self::Removed,
      wvb::source::ManifestRemoveResultKind::NotExists => Self::NotExists,
      wvb::source::ManifestRemoveResultKind::VersionNotExists => Self::VersionNotExists,
      wvb::source::ManifestRemoveResultKind::InUse => Self::InUse,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRemoveResult {
  pub name: String,
  pub version: String,
  pub kind: ManifestRemoveResultKind,
}

impl From<wvb::source::ManifestRemoveResult> for ManifestRemoveResult {
  fn from(value: wvb::source::ManifestRemoveResult) -> Self {
    Self {
      name: value.name,
      version: value.version,
      kind: value.kind.into(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestPruneResult {
  pub name: String,
  pub pruned_versions: Vec<String>,
}

impl From<wvb::source::ManifestPruneResult> for ManifestPruneResult {
  fn from(value: wvb::source::ManifestPruneResult) -> Self {
    Self {
      name: value.name,
      pruned_versions: value.pruned_versions,
    }
  }
}

#[command]
pub async fn source_list_bundles<R: Runtime>(
  app: AppHandle<R>,
) -> crate::Result<Vec<ManifestListItem>> {
  let wvb = app.wvb();
  let items = wvb
    .source()
    .list_bundles()
    .await?
    .into_iter()
    .map(ManifestListItem::from)
    .collect::<Vec<_>>();
  Ok(items)
}

#[command]
pub async fn source_list_builtin_bundles<R: Runtime>(
  app: AppHandle<R>,
) -> crate::Result<Vec<ManifestListItem>> {
  let wvb = app.wvb();
  let items = wvb
    .source()
    .list_builtin_bundles()
    .await?
    .into_iter()
    .map(ManifestListItem::from)
    .collect::<Vec<_>>();
  Ok(items)
}

#[command]
pub async fn source_list_remote_bundles<R: Runtime>(
  app: AppHandle<R>,
) -> crate::Result<Vec<ManifestListItem>> {
  let wvb = app.wvb();
  let items = wvb
    .source()
    .list_remote_bundles()
    .await?
    .into_iter()
    .map(ManifestListItem::from)
    .collect::<Vec<_>>();
  Ok(items)
}

#[command]
pub async fn source_get_version<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
) -> crate::Result<Option<BundleSourceVersion>> {
  let wvb = app.wvb();
  let version = wvb
    .source()
    .get_version(&bundle_name)
    .await?
    .map(BundleSourceVersion::from);
  Ok(version)
}

#[command]
pub async fn source_get_remote_staged_version<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
) -> crate::Result<Option<String>> {
  let wvb = app.wvb();
  let version = wvb.source().get_remote_staged_version(&bundle_name).await?;
  Ok(version)
}

#[command]
pub async fn source_get_remote_previous_version<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
) -> crate::Result<Option<String>> {
  let wvb = app.wvb();
  let version = wvb
    .source()
    .get_remote_previous_version(&bundle_name)
    .await?;
  Ok(version)
}

#[command]
pub async fn source_get_builtin_version_data<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
) -> crate::Result<Option<ManifestVersionData>> {
  let wvb = app.wvb();
  let data = wvb
    .source()
    .get_builtin_version_data(&bundle_name, &version)
    .await?
    .map(ManifestVersionData::from);
  Ok(data)
}

#[command]
pub async fn source_get_remote_version_data<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
) -> crate::Result<Option<ManifestVersionData>> {
  let wvb = app.wvb();
  let data = wvb
    .source()
    .get_remote_version_data(&bundle_name, &version)
    .await?
    .map(ManifestVersionData::from);
  Ok(data)
}

#[command]
pub async fn source_update_remote_version<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
) -> crate::Result<ManifestSetCurrentVersionResult> {
  let wvb = app.wvb();
  let result = wvb
    .source()
    .update_remote_version(&bundle_name, &version)
    .await?;
  Ok(result.into())
}

#[command]
pub async fn source_update_remote_versions<R: Runtime>(
  app: AppHandle<R>,
  items: HashMap<String, String>,
) -> crate::Result<Vec<ManifestSetCurrentVersionResult>> {
  let wvb = app.wvb();
  let results = wvb
    .source()
    .update_remote_versions(items)
    .await?
    .into_iter()
    .map(ManifestSetCurrentVersionResult::from)
    .collect::<Vec<_>>();
  Ok(results)
}

#[command]
pub async fn source_stage_remote_bundle<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  data: ManifestStageData,
) -> crate::Result<ManifestStageResult> {
  let wvb = app.wvb();
  let result = wvb
    .source()
    .stage_remote_bundle(&bundle_name, data.into())
    .await?;
  Ok(result.into())
}

#[command]
pub async fn source_stage_remote_bundles<R: Runtime>(
  app: AppHandle<R>,
  items: HashMap<String, ManifestStageData>,
) -> crate::Result<Vec<ManifestStageResult>> {
  let wvb = app.wvb();
  let items = items
    .into_iter()
    .map(|(name, data)| (name, data.into()))
    .collect::<HashMap<String, wvb::source::ManifestStageData>>();
  let results = wvb
    .source()
    .stage_remote_bundles(items)
    .await?
    .into_iter()
    .map(ManifestStageResult::from)
    .collect::<Vec<_>>();
  Ok(results)
}

#[command]
pub async fn source_remove_remote_bundle<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
  force: Option<bool>,
) -> crate::Result<ManifestRemoveResult> {
  let wvb = app.wvb();
  let result = wvb
    .source()
    .remove_remote_bundle(&bundle_name, &version, force)
    .await?;
  Ok(result.into())
}

#[command]
pub async fn source_remove_remote_bundles<R: Runtime>(
  app: AppHandle<R>,
  items: HashMap<String, ManifestRemoveData>,
) -> crate::Result<Vec<ManifestRemoveResult>> {
  let wvb = app.wvb();
  let items = items
    .into_iter()
    .map(|(name, data)| (name, data.into()))
    .collect::<HashMap<String, wvb::source::ManifestRemoveData>>();
  let results = wvb
    .source()
    .remove_remote_bundles(items)
    .await?
    .into_iter()
    .map(ManifestRemoveResult::from)
    .collect::<Vec<_>>();
  Ok(results)
}

#[command]
pub async fn source_prune_remote_bundle<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
) -> crate::Result<ManifestPruneResult> {
  let wvb = app.wvb();
  let result = wvb.source().prune_remote_bundle(&bundle_name).await?;
  Ok(result.into())
}

#[command]
pub async fn source_prune_remote_bundles<R: Runtime>(
  app: AppHandle<R>,
  bundle_names: Vec<String>,
) -> crate::Result<Vec<ManifestPruneResult>> {
  let wvb = app.wvb();
  let results = wvb
    .source()
    .prune_remote_bundles(&bundle_names)
    .await?
    .into_iter()
    .map(ManifestPruneResult::from)
    .collect::<Vec<_>>();
  Ok(results)
}

#[command]
pub async fn source_resolve_filepath<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
) -> crate::Result<String> {
  let wvb = app.wvb();
  let filepath = wvb.source().resolve_filepath(&bundle_name).await?;
  Ok(filepath.to_string_lossy().to_string())
}

#[command]
pub async fn source_get_builtin_bundle_filepath<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
) -> crate::Result<String> {
  let wvb = app.wvb();
  let filepath = wvb
    .source()
    .get_builtin_bundle_filepath(&bundle_name, &version)?;
  Ok(filepath.to_string_lossy().to_string())
}

#[command]
pub async fn source_get_remote_bundle_filepath<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
) -> crate::Result<String> {
  let wvb = app.wvb();
  let filepath = wvb
    .source()
    .get_remote_bundle_filepath(&bundle_name, &version)?;
  Ok(filepath.to_string_lossy().to_string())
}

#[command]
pub async fn source_unload<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
) -> crate::Result<bool> {
  let wvb = app.wvb();
  Ok(wvb.source().unload(&bundle_name))
}
