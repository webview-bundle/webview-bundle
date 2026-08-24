use crate::WebviewBundleExtra;
use crate::command::remote::{BundleUpdate, Update};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Runtime, command};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterGetUpdateOptions {
  pub expect_signature_key_id: Option<String>,
}

impl From<UpdaterGetUpdateOptions> for wvb::updater::UpdaterGetUpdateOptions {
  fn from(value: UpdaterGetUpdateOptions) -> Self {
    let mut options = wvb::updater::UpdaterGetUpdateOptions::default();
    if let Some(key_id) = value.expect_signature_key_id {
      options = options.expect_signature_key_id(key_id);
    }
    options
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterDownloadOptions {
  pub concurrency: Option<usize>,
  pub timeout: Option<u64>,
}

impl From<UpdaterDownloadOptions> for wvb::updater::UpdaterDownloadOptions {
  fn from(value: UpdaterDownloadOptions) -> Self {
    let mut options = wvb::updater::UpdaterDownloadOptions::default();
    if let Some(concurrency) = value.concurrency {
      options = options.concurrency(concurrency);
    }
    if let Some(timeout) = value.timeout {
      options = options.timeout(timeout);
    }
    options
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdaterDownloadResultKind {
  Downloaded,
  Error { code: String, message: String },
}

impl From<wvb::updater::UpdaterDownloadResultKind> for UpdaterDownloadResultKind {
  fn from(value: wvb::updater::UpdaterDownloadResultKind) -> Self {
    match value {
      wvb::updater::UpdaterDownloadResultKind::Downloaded => Self::Downloaded,
      wvb::updater::UpdaterDownloadResultKind::Error(error) => Self::Error {
        code: error.code().as_str().to_owned(),
        message: error.to_string(),
      },
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterDownloadResult {
  pub name: String,
  pub version: String,
  pub integrity: Option<String>,
  pub metadata: Option<HashMap<String, String>>,
  pub result: UpdaterDownloadResultKind,
}

impl From<wvb::updater::UpdaterDownloadResult> for UpdaterDownloadResult {
  fn from(value: wvb::updater::UpdaterDownloadResult) -> Self {
    Self {
      name: value.name,
      version: value.version,
      integrity: value.integrity,
      metadata: value.metadata,
      result: value.result.into(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterInstallTarget {
  pub name: String,
  pub version: Option<String>,
}

impl From<UpdaterInstallTarget> for wvb::updater::UpdaterInstallTarget {
  fn from(value: UpdaterInstallTarget) -> Self {
    Self {
      name: value.name,
      version: value.version,
    }
  }
}

impl From<wvb::updater::UpdaterInstallTarget> for UpdaterInstallTarget {
  fn from(value: wvb::updater::UpdaterInstallTarget) -> Self {
    Self {
      name: value.name,
      version: value.version,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdaterInstallResultKind {
  Installed,
  StagedVersionNotMatched,
  StagedBundleNotExists,
  VerifyFailed,
  Error { code: String, message: String },
}

impl From<wvb::updater::UpdaterInstallResultKind> for UpdaterInstallResultKind {
  fn from(value: wvb::updater::UpdaterInstallResultKind) -> Self {
    match value {
      wvb::updater::UpdaterInstallResultKind::Installed => Self::Installed,
      wvb::updater::UpdaterInstallResultKind::StagedVersionNotMatched => {
        Self::StagedVersionNotMatched
      }
      wvb::updater::UpdaterInstallResultKind::StagedBundleNotExists => Self::StagedBundleNotExists,
      wvb::updater::UpdaterInstallResultKind::VerifyFailed => Self::VerifyFailed,
      wvb::updater::UpdaterInstallResultKind::Error(error) => Self::Error {
        code: error.code().as_str().to_owned(),
        message: error.to_string(),
      },
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterInstallResult {
  pub name: String,
  pub target_version: Option<String>,
  pub install_version: Option<String>,
  pub result: UpdaterInstallResultKind,
}

impl From<wvb::updater::UpdaterInstallResult> for UpdaterInstallResult {
  fn from(value: wvb::updater::UpdaterInstallResult) -> Self {
    Self {
      name: value.name,
      target_version: value.target_version,
      install_version: value.install_version,
      result: value.result.into(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterRollbackTarget {
  pub name: String,
  pub version: Option<String>,
}

impl From<UpdaterRollbackTarget> for wvb::updater::UpdaterRollbackTarget {
  fn from(value: UpdaterRollbackTarget) -> Self {
    Self {
      name: value.name,
      version: value.version,
    }
  }
}

impl From<wvb::updater::UpdaterRollbackTarget> for UpdaterRollbackTarget {
  fn from(value: wvb::updater::UpdaterRollbackTarget) -> Self {
    Self {
      name: value.name,
      version: value.version,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdaterRollbackResultKind {
  RolledBack,
  PreviousVersionNotMatched,
  PreviousBundleNotExists,
  VerifyFailed,
  Error { code: String, message: String },
}

impl From<wvb::updater::UpdaterRollbackResultKind> for UpdaterRollbackResultKind {
  fn from(value: wvb::updater::UpdaterRollbackResultKind) -> Self {
    match value {
      wvb::updater::UpdaterRollbackResultKind::RolledBack => Self::RolledBack,
      wvb::updater::UpdaterRollbackResultKind::PreviousVersionNotMatched => {
        Self::PreviousVersionNotMatched
      }
      wvb::updater::UpdaterRollbackResultKind::PreviousBundleNotExists => {
        Self::PreviousBundleNotExists
      }
      wvb::updater::UpdaterRollbackResultKind::VerifyFailed => Self::VerifyFailed,
      wvb::updater::UpdaterRollbackResultKind::Error(error) => Self::Error {
        code: error.code().as_str().to_owned(),
        message: error.to_string(),
      },
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterRollbackResult {
  pub name: String,
  pub target_version: Option<String>,
  pub rollback_version: Option<String>,
  pub result: UpdaterRollbackResultKind,
}

impl From<wvb::updater::UpdaterRollbackResult> for UpdaterRollbackResult {
  fn from(value: wvb::updater::UpdaterRollbackResult) -> Self {
    Self {
      name: value.name,
      target_version: value.target_version,
      rollback_version: value.rollback_version,
      result: value.result.into(),
    }
  }
}

#[command]
pub async fn updater_get_update<R: Runtime>(
  app: AppHandle<R>,
  options: Option<UpdaterGetUpdateOptions>,
) -> crate::Result<Option<Update>> {
  let wvb = app.wvb();
  let update = wvb
    .updater()
    .ok_or(crate::Error::UpdaterIsNotInitialized)?
    .get_update(options.map(Into::into))
    .await?
    .map(Update::from);
  Ok(update)
}

#[command]
pub async fn updater_download<R: Runtime>(
  app: AppHandle<R>,
  bundle_updates: Vec<BundleUpdate>,
  options: Option<UpdaterDownloadOptions>,
) -> crate::Result<Vec<UpdaterDownloadResult>> {
  let wvb = app.wvb();
  let bundle_updates = bundle_updates
    .into_iter()
    .map(wvb::remote::BundleUpdate::from)
    .collect::<Vec<_>>();
  let results = wvb
    .updater()
    .ok_or(crate::Error::UpdaterIsNotInitialized)?
    .download(&bundle_updates, options.map(Into::into))
    .await?
    .into_iter()
    .map(UpdaterDownloadResult::from)
    .collect::<Vec<_>>();
  Ok(results)
}

#[command]
pub async fn updater_install<R: Runtime>(
  app: AppHandle<R>,
  targets: Vec<UpdaterInstallTarget>,
) -> crate::Result<Vec<UpdaterInstallResult>> {
  let wvb = app.wvb();
  let targets = targets
    .into_iter()
    .map(wvb::updater::UpdaterInstallTarget::from)
    .collect::<Vec<_>>();
  let results = wvb
    .updater()
    .ok_or(crate::Error::UpdaterIsNotInitialized)?
    .install(&targets)
    .await?
    .into_iter()
    .map(UpdaterInstallResult::from)
    .collect::<Vec<_>>();
  Ok(results)
}

#[command]
pub async fn updater_rollback<R: Runtime>(
  app: AppHandle<R>,
  targets: Vec<UpdaterRollbackTarget>,
) -> crate::Result<Vec<UpdaterRollbackResult>> {
  let wvb = app.wvb();
  let targets = targets
    .into_iter()
    .map(wvb::updater::UpdaterRollbackTarget::from)
    .collect::<Vec<_>>();
  let results = wvb
    .updater()
    .ok_or(crate::Error::UpdaterIsNotInitialized)?
    .rollback(&targets)
    .await?
    .into_iter()
    .map(UpdaterRollbackResult::from)
    .collect::<Vec<_>>();
  Ok(results)
}
