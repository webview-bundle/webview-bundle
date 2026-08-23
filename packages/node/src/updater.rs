use crate::cancellation::Cancellation;
use crate::error::ErrorCode;
use crate::integrity::{IntegrityAlgorithm, IntegrityPolicy};
use crate::remote::{BundleUpdate, Remote, Update};
use crate::signature::SignatureVerifyKey;
use crate::source::Source;
use napi::Env;
use napi_derive::napi;
use std::collections::HashMap;
use std::path::Path;
use wvb::updater;

#[napi(object, object_to_js = false)]
pub struct UpdaterIntegrityOptions {
  pub policy: Option<IntegrityPolicy>,
  pub algorithm: Option<IntegrityAlgorithm>,
}

impl From<UpdaterIntegrityOptions> for updater::UpdaterIntegrityOptions {
  fn from(value: UpdaterIntegrityOptions) -> Self {
    let mut options = updater::UpdaterIntegrityOptions::default();
    if let Some(policy) = value.policy {
      options = options.policy(policy.into());
    }
    if let Some(algorithm) = value.algorithm {
      options = options.algorithm(algorithm.into());
    }
    options
  }
}

#[napi(object, object_to_js = false)]
pub struct UpdaterSignatureOptions {
  pub keys: Option<Vec<SignatureVerifyKey>>,
}

impl From<UpdaterSignatureOptions> for updater::UpdaterSignatureOptions {
  fn from(value: UpdaterSignatureOptions) -> Self {
    let mut options = updater::UpdaterSignatureOptions::default();
    if let Some(key_sets) = value.keys {
      options = options.add_keys(key_sets.into_iter().map(Into::into).collect::<Vec<_>>());
    }
    options
  }
}

#[napi(object, object_to_js = false)]
pub struct UpdaterOptions {
  pub channel: Option<String>,
  pub integrity: Option<UpdaterIntegrityOptions>,
  pub signature: Option<UpdaterSignatureOptions>,
}

impl From<UpdaterOptions> for updater::UpdaterOptions {
  fn from(value: UpdaterOptions) -> Self {
    let mut options = updater::UpdaterOptions::default();
    if let Some(channel) = value.channel {
      options = options.channel(channel);
    }
    if let Some(integrity) = value.integrity {
      options = options.integrity(integrity.into());
    }
    if let Some(signature) = value.signature {
      options = options.signature(signature.into());
    }
    options
  }
}

#[napi(object, object_to_js = false)]
pub struct UpdaterGetUpdateOptions {
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

#[napi(object, object_to_js = false)]
pub struct UpdaterDownloadOptions {
  pub concurrency: Option<u32>,
  pub timeout: Option<u32>,
}

#[napi(discriminant_case = "snake_case", object_from_js = false)]
pub enum UpdaterDownloadResultKind {
  Downloaded,
  Error { code: ErrorCode, message: String },
}

#[napi(object, object_from_js = false)]
pub struct UpdaterDownloadResult {
  pub name: String,
  pub version: String,
  pub integrity: Option<String>,
  pub metadata: Option<HashMap<String, String>>,
  pub result: UpdaterDownloadResultKind,
}

impl From<updater::UpdaterDownloadResult> for UpdaterDownloadResult {
  fn from(value: updater::UpdaterDownloadResult) -> Self {
    let result = match value.result {
      updater::UpdaterDownloadResultKind::Downloaded => UpdaterDownloadResultKind::Downloaded,
      updater::UpdaterDownloadResultKind::Error(e) => UpdaterDownloadResultKind::Error {
        code: e.code().into(),
        message: e.to_string(),
      },
    };
    Self {
      name: value.name,
      version: value.version,
      integrity: value.integrity,
      metadata: value.metadata,
      result,
    }
  }
}

#[napi(object)]
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

impl From<updater::UpdaterInstallTarget> for UpdaterInstallTarget {
  fn from(value: updater::UpdaterInstallTarget) -> Self {
    Self {
      name: value.name,
      version: value.version,
    }
  }
}

#[napi(discriminant_case = "snake_case", object_from_js = false)]
pub enum UpdaterInstallResultKind {
  Installed,
  StagedVersionNotMatched,
  StagedBundleNotExists,
  VerifyFailed,
  Error { code: ErrorCode, message: String },
}

impl From<updater::UpdaterInstallResultKind> for UpdaterInstallResultKind {
  fn from(value: updater::UpdaterInstallResultKind) -> Self {
    match value {
      updater::UpdaterInstallResultKind::Installed => Self::Installed,
      updater::UpdaterInstallResultKind::StagedVersionNotMatched => Self::StagedVersionNotMatched,
      updater::UpdaterInstallResultKind::StagedBundleNotExists => Self::StagedBundleNotExists,
      updater::UpdaterInstallResultKind::VerifyFailed => Self::VerifyFailed,
      updater::UpdaterInstallResultKind::Error(error) => Self::Error {
        code: error.code().into(),
        message: error.to_string(),
      },
    }
  }
}

#[napi(object, object_from_js = false)]
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

#[napi(object)]
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

impl From<updater::UpdaterRollbackTarget> for UpdaterRollbackTarget {
  fn from(value: updater::UpdaterRollbackTarget) -> Self {
    Self {
      name: value.name,
      version: value.version,
    }
  }
}

#[napi(discriminant_case = "snake_case", object_from_js = false)]
pub enum UpdaterRollbackResultKind {
  RolledBack,
  PreviousVersionNotMatched,
  PreviousBundleNotExists,
  VerifyFailed,
  GroupFailed { groups: Vec<String> },
  Error { code: ErrorCode, message: String },
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
      updater::UpdaterRollbackResultKind::Error(error) => Self::Error {
        code: error.code().into(),
        message: error.to_string(),
      },
    }
  }
}

#[napi(object, object_from_js = false)]
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

#[napi]
pub struct Updater {
  pub(crate) inner: updater::Updater,
}

#[napi]
impl Updater {
  #[napi(constructor)]
  pub fn new(
    source: &Source,
    remote: &Remote,
    update_filepath: String,
    options: Option<UpdaterOptions>,
    env: Env,
  ) -> napi::Result<Updater> {
    crate::Outcome::from_fn(|| {
      let mut builder = updater::Updater::builder()
        .source(source.inner.clone())
        .remote(remote.inner.clone())
        .update_filepath(Path::new(&update_filepath));
      if let Some(options) = options {
        builder = builder.options(options.into());
      }
      Ok(Updater {
        inner: builder.build()?,
      })
    })
    .into_napi(env)
  }

  #[napi(ts_return_type = "Promise<Update | null>")]
  pub async fn get_update(
    &self,
    options: Option<UpdaterGetUpdateOptions>,
  ) -> crate::Outcome<Option<Update>> {
    crate::Outcome::from_future(async {
      let update = self
        .inner
        .get_update(options.map(Into::into))
        .await?
        .map(Update::from);
      Ok(update)
    })
    .await
  }

  #[napi(ts_return_type = "Promise<UpdaterDownloadResult[]>")]
  pub async fn download(
    &self,
    bundle_updates: Vec<BundleUpdate>,
    options: Option<UpdaterDownloadOptions>,
    cancellation: Option<&Cancellation>,
  ) -> crate::Outcome<Vec<UpdaterDownloadResult>> {
    crate::Outcome::from_future(async {
      let bundle_updates = bundle_updates
        .into_iter()
        .map(Into::into)
        .collect::<Vec<wvb::remote::BundleUpdate>>();

      let mut download_options = updater::UpdaterDownloadOptions::default();
      if let Some(options) = options {
        if let Some(concurrency) = options.concurrency {
          download_options = download_options.concurrency(concurrency as usize);
        }
        if let Some(timeout) = options.timeout {
          download_options = download_options.timeout(timeout as u64);
        }
      }
      if let Some(cancellation) = cancellation {
        download_options = download_options.cancellation(cancellation.inner.clone());
      }

      let results = self
        .inner
        .download(&bundle_updates, Some(download_options))
        .await?
        .into_iter()
        .map(UpdaterDownloadResult::from)
        .collect::<Vec<_>>();
      Ok(results)
    })
    .await
  }

  #[napi(ts_return_type = "Promise<UpdaterInstallResult[]>")]
  pub async fn install(
    &self,
    targets: Vec<UpdaterInstallTarget>,
  ) -> crate::Outcome<Vec<UpdaterInstallResult>> {
    crate::Outcome::from_future(async {
      let targets = targets
        .into_iter()
        .map(Into::into)
        .collect::<Vec<updater::UpdaterInstallTarget>>();
      let results = self
        .inner
        .install(&targets)
        .await?
        .into_iter()
        .map(UpdaterInstallResult::from)
        .collect::<Vec<_>>();
      Ok(results)
    })
    .await
  }

  #[napi(ts_return_type = "Promise<UpdaterRollbackResult[]>")]
  pub async fn rollback(
    &self,
    targets: Vec<UpdaterRollbackTarget>,
  ) -> crate::Outcome<Vec<UpdaterRollbackResult>> {
    crate::Outcome::from_future(async {
      let targets = targets
        .into_iter()
        .map(Into::into)
        .collect::<Vec<updater::UpdaterRollbackTarget>>();
      let results = self
        .inner
        .rollback(&targets)
        .await?
        .into_iter()
        .map(UpdaterRollbackResult::from)
        .collect::<Vec<_>>();
      Ok(results)
    })
    .await
  }
}
