#[cfg(feature = "integrity")]
use crate::integrity;
use crate::remote::BundleUpdate;
#[cfg(feature = "signature")]
use crate::signature;
use crate::util::cancellation::Cancellation;
use std::collections::HashMap;

/// How bundles are checked against the integrity recorded for them
/// in the remote manifest.
#[cfg(feature = "integrity")]
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct UpdaterIntegrityOptions {
  pub policy: integrity::IntegrityPolicy,
  pub algorithm: Option<integrity::IntegrityAlgorithm>,
}

#[cfg(feature = "integrity")]
impl UpdaterIntegrityOptions {
  pub fn policy(mut self, policy: integrity::IntegrityPolicy) -> Self {
    self.policy = policy;
    self
  }

  pub fn algorithm(mut self, alg: integrity::IntegrityAlgorithm) -> Self {
    self.algorithm = Some(alg);
    self
  }
}

/// Signature options for verify update information from remote server.
#[cfg(feature = "signature")]
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct UpdaterSignatureOptions {
  pub keys: Option<Vec<signature::SignatureVerifyKey>>,
}

#[cfg(feature = "signature")]
impl UpdaterSignatureOptions {
  pub fn add_key(self, key_set: signature::SignatureVerifyKey) -> Self {
    self.add_keys(vec![key_set])
  }

  pub fn add_keys(mut self, key_sets: Vec<signature::SignatureVerifyKey>) -> Self {
    self.keys = match self.keys {
      Some(original) => Some([original, key_sets].concat()),
      None => Some(key_sets),
    };
    self
  }
}

#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct UpdaterOptions {
  /// Optional channel to fetch updates from remote server.
  pub channel: Option<String>,
  #[cfg(feature = "integrity")]
  /// Integrity options for downloaded bundle.
  pub integrity: UpdaterIntegrityOptions,
  #[cfg(feature = "signature")]
  /// Optional updater signature options.
  /// It is used to verify update information retrieved from a remote server.
  ///
  /// This is recommended in production environments.
  pub signature: UpdaterSignatureOptions,
}

impl UpdaterOptions {
  pub fn channel(mut self, channel: impl Into<String>) -> Self {
    self.channel = Some(channel.into());
    self
  }

  #[cfg(feature = "integrity")]
  pub fn integrity(mut self, integrity: UpdaterIntegrityOptions) -> Self {
    self.integrity = integrity;
    self
  }

  #[cfg(feature = "signature")]
  pub fn signature(mut self, signature: UpdaterSignatureOptions) -> Self {
    self.signature = signature;
    self
  }
}

#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct UpdaterGetUpdateOptions {
  #[cfg(feature = "signature")]
  pub expect_signature_key_id: Option<String>,
}

impl UpdaterGetUpdateOptions {
  /// Requires the update response to be signed by the key published under `key_id`, which
  /// must be one of the key sets the updater was configured with.
  #[cfg(feature = "signature")]
  pub fn expect_signature_key_id(mut self, key_id: impl Into<String>) -> Self {
    self.expect_signature_key_id = Some(key_id.into());
    self
  }
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct UpdaterDownloadOptions {
  pub concurrency: Option<usize>,
  pub timeout: Option<u64>,
  pub cancellation: Option<Cancellation>,
}

impl UpdaterDownloadOptions {
  pub fn concurrency(mut self, concurrency: usize) -> Self {
    self.concurrency = Some(concurrency);
    self
  }

  pub fn timeout(mut self, timeout: u64) -> Self {
    self.timeout = Some(timeout);
    self
  }

  pub fn cancellation(mut self, cancellation: Cancellation) -> Self {
    self.cancellation = Some(cancellation);
    self
  }
}

#[derive(Debug)]
pub enum UpdaterDownloadResultKind {
  Downloaded,
  Error(crate::Error),
}

impl From<crate::Error> for UpdaterDownloadResultKind {
  fn from(error: crate::Error) -> Self {
    Self::Error(error)
  }
}

#[derive(Debug)]
pub(crate) struct UpdaterDownloadResultInner {
  pub update: BundleUpdate,
  pub result: crate::Result<()>,
}

impl From<UpdaterDownloadResultInner> for UpdaterDownloadResult {
  fn from(value: UpdaterDownloadResultInner) -> Self {
    Self {
      name: value.update.name,
      version: value.update.version,
      integrity: value.update.integrity,
      metadata: value.update.metadata,
      result: match value.result {
        Ok(_) => UpdaterDownloadResultKind::Downloaded,
        Err(e) => UpdaterDownloadResultKind::Error(e),
      },
    }
  }
}

#[derive(Debug)]
pub struct UpdaterDownloadResult {
  pub name: String,
  pub version: String,
  pub integrity: Option<String>,
  pub metadata: Option<HashMap<String, String>>,
  pub result: UpdaterDownloadResultKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdaterInstallTarget {
  pub name: String,
  /// The staged version to install.
  /// If this is not set, staged version recorded in the manifest will be used.
  /// If this is set, will match the staged version recorded in the manifest.
  pub version: Option<String>,
}

#[derive(Debug)]
pub enum UpdaterInstallResultKind {
  Installed,
  StagedVersionNotMatched,
  StagedBundleNotExists,
  VerifyFailed,
  Error(crate::Error),
}

impl From<crate::Error> for UpdaterInstallResultKind {
  fn from(error: crate::Error) -> Self {
    Self::Error(error)
  }
}

#[derive(Debug)]
pub struct UpdaterInstallResult {
  pub name: String,
  pub target_version: Option<String>,
  pub install_version: Option<String>,
  pub result: UpdaterInstallResultKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdaterRollbackTarget {
  pub name: String,
  /// The previous version to roll back to.
  /// If this is not set, previous version recorded in the manifest will be used.
  /// If this is set, will match the previous version recorded in the manifest.
  pub version: Option<String>,
}

#[derive(Debug)]
pub enum UpdaterRollbackResultKind {
  RolledBack,
  PreviousVersionNotMatched,
  PreviousBundleNotExists,
  VerifyFailed,
  Error(crate::Error),
}

impl From<crate::Error> for UpdaterRollbackResultKind {
  fn from(error: crate::Error) -> Self {
    Self::Error(error)
  }
}

#[derive(Debug)]
pub struct UpdaterRollbackResult {
  pub name: String,
  pub target_version: Option<String>,
  pub rollback_version: Option<String>,
  pub result: UpdaterRollbackResultKind,
}
