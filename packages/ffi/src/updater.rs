use crate::cancellation::Cancellation;
use crate::integrity::{IntegrityAlgorithm, IntegrityPolicy};
use crate::remote::{BundleUpdate, Remote, Update};
use crate::signature::SignatureVerifyKey;
use crate::source::Source;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use wvb::signature;
use wvb::updater;

#[derive(uniffi::Record, Clone, Debug)]
pub struct UpdaterIntegrityOptions {
  /// How a bundle's integrity metadata is treated (default: [`IntegrityPolicy::Optional`]).
  ///
  /// [`IntegrityPolicy::Off`] disables the integrity check entirely.
  #[uniffi(default = None)]
  pub policy: Option<IntegrityPolicy>,
  #[uniffi(default = None)]
  pub algorithm: Option<IntegrityAlgorithm>,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct UpdaterSignatureOptions {
  #[uniffi(default = None)]
  pub keys: Option<Vec<SignatureVerifyKey>>,
}

/// Optional configuration for the [`Updater`].
#[derive(uniffi::Record, Clone, Debug)]
pub struct UpdaterOptions {
  /// Release channel (e.g. `"stable"`, `"beta"`). Passed as a query parameter to the remote.
  #[uniffi(default = None)]
  pub channel: Option<String>,
  #[uniffi(default = None)]
  pub integrity: Option<UpdaterIntegrityOptions>,
  #[uniffi(default = None)]
  pub signature: Option<UpdaterSignatureOptions>,
}

fn updater_options(options: UpdaterOptions) -> crate::Result<updater::UpdaterOptions> {
  let mut updater_options = updater::UpdaterOptions::default();

  if let Some(channel) = options.channel {
    updater_options = updater_options.channel(channel);
  }

  if let Some(integrity) = options.integrity {
    let mut integrity_options = updater::UpdaterIntegrityOptions::default();
    if let Some(policy) = integrity.policy {
      integrity_options = integrity_options.policy(policy.into());
    }
    if let Some(algorithm) = integrity.algorithm {
      integrity_options = integrity_options.algorithm(algorithm.into());
    }
    updater_options = updater_options.integrity(integrity_options);
  }

  if let Some(signature) = options.signature {
    let mut signature_options = updater::UpdaterSignatureOptions::default();
    if let Some(keys) = signature.keys {
      for key in keys {
        signature_options =
          signature_options.add_key(signature::SignatureVerifyKey::try_from(key)?);
      }
    }
    updater_options = updater_options.signature(signature_options);
  }

  Ok(updater_options)
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct UpdaterGetUpdateOptions {
  #[uniffi(default = None)]
  pub expect_signature_key_id: Option<String>,
}

impl From<UpdaterGetUpdateOptions> for updater::UpdaterGetUpdateOptions {
  fn from(value: UpdaterGetUpdateOptions) -> Self {
    let mut options = updater::UpdaterGetUpdateOptions::default();
    if let Some(key_id) = value.expect_signature_key_id {
      options = options.expect_signature_key_id(key_id);
    }
    options
  }
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct UpdaterDownloadOptions {
  #[uniffi(default = None)]
  pub concurrency: Option<u32>,
  #[uniffi(default = None)]
  pub timeout: Option<u64>,
  #[uniffi(default = None)]
  pub cancellation: Option<Arc<Cancellation>>,
}

impl From<UpdaterDownloadOptions> for updater::UpdaterDownloadOptions {
  fn from(value: UpdaterDownloadOptions) -> Self {
    let mut options = updater::UpdaterDownloadOptions::default();
    if let Some(concurrency) = value.concurrency {
      options = options.concurrency(concurrency as usize);
    }
    if let Some(timeout) = value.timeout {
      options = options.timeout(timeout);
    }
    if let Some(cancellation) = value.cancellation {
      options = options.cancellation(cancellation.inner.clone());
    }
    options
  }
}

#[derive(uniffi::Enum, Debug)]
pub enum UpdaterDownloadResultKind {
  Downloaded,
  Error(crate::Error),
}

impl From<updater::UpdaterDownloadResultKind> for UpdaterDownloadResultKind {
  fn from(value: updater::UpdaterDownloadResultKind) -> Self {
    match value {
      updater::UpdaterDownloadResultKind::Downloaded => Self::Downloaded,
      updater::UpdaterDownloadResultKind::Error(e) => Self::Error(crate::Error::from(e)),
    }
  }
}

#[derive(uniffi::Record, Debug)]
pub struct UpdaterDownloadResult {
  pub name: String,
  pub version: String,
  pub integrity: Option<String>,
  pub metadata: Option<HashMap<String, String>>,
  pub result: UpdaterDownloadResultKind,
}

impl From<updater::UpdaterDownloadResult> for UpdaterDownloadResult {
  fn from(value: updater::UpdaterDownloadResult) -> Self {
    Self {
      name: value.name,
      version: value.version,
      integrity: value.integrity,
      metadata: value.metadata,
      result: value.result.into(),
    }
  }
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct UpdaterInstallTarget {
  pub name: String,
  pub version: Option<String>,
}

impl From<UpdaterInstallTarget> for updater::UpdaterInstallTarget {
  fn from(value: UpdaterInstallTarget) -> Self {
    Self {
      name: value.name,
      version: value.version,
    }
  }
}

#[derive(uniffi::Enum, Debug)]
pub enum UpdaterInstallResultKind {
  Installed,
  StagedVersionNotMatched,
  StagedBundleNotExists,
  VerifyFailed,
  Error(crate::Error),
}

impl From<updater::UpdaterInstallResultKind> for UpdaterInstallResultKind {
  fn from(value: updater::UpdaterInstallResultKind) -> Self {
    match value {
      updater::UpdaterInstallResultKind::Installed => Self::Installed,
      updater::UpdaterInstallResultKind::StagedVersionNotMatched => Self::StagedVersionNotMatched,
      updater::UpdaterInstallResultKind::StagedBundleNotExists => Self::StagedBundleNotExists,
      updater::UpdaterInstallResultKind::VerifyFailed => Self::VerifyFailed,
      updater::UpdaterInstallResultKind::Error(e) => Self::Error(e.into()),
    }
  }
}

#[derive(uniffi::Record, Debug)]
pub struct UpdaterInstallResult {
  pub name: String,
  pub target_version: Option<String>,
  pub install_version: Option<String>,
  pub result: UpdaterInstallResultKind,
}

impl From<updater::UpdaterInstallResult> for UpdaterInstallResult {
  fn from(value: updater::UpdaterInstallResult) -> Self {
    Self {
      name: value.name,
      target_version: value.target_version,
      install_version: value.install_version,
      result: value.result.into(),
    }
  }
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct UpdaterRollbackTarget {
  pub name: String,
  pub version: Option<String>,
}

impl From<UpdaterRollbackTarget> for updater::UpdaterRollbackTarget {
  fn from(value: UpdaterRollbackTarget) -> Self {
    Self {
      name: value.name,
      version: value.version,
    }
  }
}

#[derive(uniffi::Enum, Debug)]
pub enum UpdaterRollbackResultKind {
  RolledBack,
  PreviousVersionNotMatched,
  PreviousBundleNotExists,
  VerifyFailed,
  Error(crate::Error),
}

impl From<updater::UpdaterRollbackResultKind> for UpdaterRollbackResultKind {
  fn from(value: updater::UpdaterRollbackResultKind) -> Self {
    match value {
      updater::UpdaterRollbackResultKind::RolledBack => Self::RolledBack,
      updater::UpdaterRollbackResultKind::PreviousVersionNotMatched => {
        Self::PreviousVersionNotMatched
      }
      updater::UpdaterRollbackResultKind::PreviousBundleNotExists => Self::PreviousBundleNotExists,
      updater::UpdaterRollbackResultKind::VerifyFailed => Self::VerifyFailed,
      updater::UpdaterRollbackResultKind::Error(e) => Self::Error(e.into()),
    }
  }
}

#[derive(uniffi::Record, Debug)]
pub struct UpdaterRollbackResult {
  pub name: String,
  pub target_version: Option<String>,
  pub rollback_version: Option<String>,
  pub result: UpdaterRollbackResultKind,
}

impl From<updater::UpdaterRollbackResult> for UpdaterRollbackResult {
  fn from(value: updater::UpdaterRollbackResult) -> Self {
    Self {
      name: value.name,
      target_version: value.target_version,
      rollback_version: value.rollback_version,
      result: value.result.into(),
    }
  }
}

/// Orchestrates the full update cycle: checks for a new version on the remote,
/// downloads it, verifies integrity/signature, and writes it to the local source.
#[derive(uniffi::Object)]
pub struct Updater {
  inner: updater::Updater,
}

#[uniffi::export]
impl Updater {
  #[uniffi::constructor]
  pub fn new(
    source: Arc<Source>,
    remote: Arc<Remote>,
    update_filepath: String,
    options: Option<UpdaterOptions>,
  ) -> crate::Result<Arc<Updater>> {
    let mut builder = updater::Updater::builder()
      .source(source.inner.clone())
      .remote(remote.inner.clone())
      .update_filepath(Path::new(&update_filepath));
    if let Some(options) = options {
      let options = updater_options(options)?;
      builder = builder.options(options);
    }
    let inner = builder.build()?;
    Ok(Arc::new(Updater { inner }))
  }
}

#[uniffi::export(async_runtime = "tokio")]
impl Updater {
  #[uniffi::method(default(options = None))]
  pub async fn get_update(
    &self,
    options: Option<UpdaterGetUpdateOptions>,
  ) -> crate::Result<Option<Update>> {
    let update = self
      .inner
      .get_update(options.map(Into::into))
      .await?
      .map(Update::from);
    Ok(update)
  }

  #[uniffi::method(default(options = None))]
  pub async fn download(
    &self,
    bundle_updates: Vec<BundleUpdate>,
    options: Option<UpdaterDownloadOptions>,
  ) -> crate::Result<Vec<UpdaterDownloadResult>> {
    let results = self
      .inner
      .download(
        &bundle_updates
          .into_iter()
          .map(Into::into)
          .collect::<Vec<_>>(),
        options.map(Into::into),
      )
      .await?
      .into_iter()
      .map(UpdaterDownloadResult::from)
      .collect::<Vec<_>>();
    Ok(results)
  }

  pub async fn install(
    &self,
    targets: Vec<UpdaterInstallTarget>,
  ) -> crate::Result<Vec<UpdaterInstallResult>> {
    let results = self
      .inner
      .install(&targets.into_iter().map(Into::into).collect::<Vec<_>>())
      .await?
      .into_iter()
      .map(UpdaterInstallResult::from)
      .collect::<Vec<_>>();
    Ok(results)
  }

  pub async fn rollback(
    &self,
    targets: Vec<UpdaterRollbackTarget>,
  ) -> crate::Result<Vec<UpdaterRollbackResult>> {
    let results = self
      .inner
      .rollback(&targets.into_iter().map(Into::into).collect::<Vec<_>>())
      .await?
      .into_iter()
      .map(UpdaterRollbackResult::from)
      .collect::<Vec<_>>();
    Ok(results)
  }
}
