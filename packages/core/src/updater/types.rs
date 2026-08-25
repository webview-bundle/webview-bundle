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
  /// Whether integrity is required, optional, or disabled.
  pub policy: integrity::IntegrityPolicy,
  /// Expected algorithm when the update advertises integrity.
  pub algorithm: Option<integrity::IntegrityAlgorithm>,
}

#[cfg(feature = "integrity")]
impl UpdaterIntegrityOptions {
  /// Sets the integrity verification policy.
  pub fn policy(mut self, policy: integrity::IntegrityPolicy) -> Self {
    self.policy = policy;
    self
  }

  /// Requires integrity values to use `alg`.
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
  /// Public keys accepted for signed update documents.
  pub keys: Option<Vec<signature::SignatureVerifyKey>>,
}

#[cfg(feature = "signature")]
impl UpdaterSignatureOptions {
  /// Adds one accepted verification key.
  pub fn add_key(self, key_set: signature::SignatureVerifyKey) -> Self {
    self.add_keys(vec![key_set])
  }

  /// Adds all accepted verification keys.
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
  /// Fetches updates from `channel`.
  pub fn channel(mut self, channel: impl Into<String>) -> Self {
    self.channel = Some(channel.into());
    self
  }

  #[cfg(feature = "integrity")]
  /// Sets the integrity verification options used during installation.
  pub fn integrity(mut self, integrity: UpdaterIntegrityOptions) -> Self {
    self.integrity = integrity;
    self
  }

  #[cfg(feature = "signature")]
  /// Sets the verification keys used for update documents.
  pub fn signature(mut self, signature: UpdaterSignatureOptions) -> Self {
    self.signature = signature;
    self
  }
}

#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct UpdaterGetUpdateOptions {
  #[cfg(feature = "signature")]
  /// Key id that must sign this response.
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
  /// Maximum number of bundle downloads running at once.
  pub concurrency: Option<usize>,
  /// Per-download timeout in milliseconds.
  pub timeout: Option<u64>,
  /// Cancellation token shared by all downloads in this operation.
  pub cancellation: Option<Cancellation>,
}

impl UpdaterDownloadOptions {
  /// Limits concurrent downloads to `concurrency`.
  pub fn concurrency(mut self, concurrency: usize) -> Self {
    self.concurrency = Some(concurrency);
    self
  }

  /// Sets the per-download timeout in milliseconds.
  pub fn timeout(mut self, timeout: u64) -> Self {
    self.timeout = Some(timeout);
    self
  }

  /// Cancels all pending downloads when `cancellation` is triggered.
  pub fn cancellation(mut self, cancellation: Cancellation) -> Self {
    self.cancellation = Some(cancellation);
    self
  }
}

#[derive(Debug)]
/// Result of downloading a single bundle.
pub enum UpdaterDownloadResultKind {
  /// The bundle was downloaded and staged successfully.
  Downloaded,
  /// Downloading or staging the bundle failed.
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
/// Per-bundle result returned by [`Updater::download`](crate::updater::Updater::download).
pub struct UpdaterDownloadResult {
  /// Name of the bundle.
  pub name: String,
  /// Version requested from the update document.
  pub version: String,
  /// Integrity value carried by the update document.
  pub integrity: Option<String>,
  /// Provider-defined bundle metadata carried by the update document.
  pub metadata: Option<HashMap<String, String>>,
  /// Download outcome.
  pub result: UpdaterDownloadResultKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A staged bundle version to activate.
pub struct UpdaterInstallTarget {
  /// Name of the bundle to install.
  pub name: String,
  /// The staged version to install.
  /// If this is not set, staged version recorded in the manifest will be used.
  /// If this is set, will match the staged version recorded in the manifest.
  pub version: Option<String>,
}

#[derive(Debug)]
/// Result of installing a single bundle.
pub enum UpdaterInstallResultKind {
  /// The staged bundle was activated.
  Installed,
  /// A version was requested but does not match the staged version.
  StagedVersionNotMatched,
  /// No staged version exists for the bundle.
  StagedBundleNotExists,
  /// Integrity or signature verification failed.
  VerifyFailed,
  /// The operation failed for another reason.
  Error(crate::Error),
}

impl From<crate::Error> for UpdaterInstallResultKind {
  fn from(error: crate::Error) -> Self {
    Self::Error(error)
  }
}

#[derive(Debug)]
/// Per-bundle result returned by [`Updater::install`](crate::updater::Updater::install).
pub struct UpdaterInstallResult {
  /// Name of the bundle.
  pub name: String,
  /// Version requested by the install target.
  pub target_version: Option<String>,
  /// Version that was activated, when installation succeeded.
  pub install_version: Option<String>,
  /// Installation outcome.
  pub result: UpdaterInstallResultKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A bundle version to restore from its recorded previous version.
pub struct UpdaterRollbackTarget {
  /// Name of the bundle to roll back.
  pub name: String,
  /// The previous version to roll back to.
  /// If this is not set, previous version recorded in the manifest will be used.
  /// If this is set, will match the previous version recorded in the manifest.
  pub version: Option<String>,
}

#[derive(Debug)]
/// Result of rolling back a single bundle.
pub enum UpdaterRollbackResultKind {
  /// The previous version was activated.
  RolledBack,
  /// A version was requested but does not match the recorded previous version.
  PreviousVersionNotMatched,
  /// No previous version exists for the bundle.
  PreviousBundleNotExists,
  /// Integrity or signature verification failed.
  VerifyFailed,
  /// The operation failed for another reason.
  Error(crate::Error),
}

impl From<crate::Error> for UpdaterRollbackResultKind {
  fn from(error: crate::Error) -> Self {
    Self::Error(error)
  }
}

#[derive(Debug)]
/// Per-bundle result returned by [`Updater::rollback`](crate::updater::Updater::rollback).
pub struct UpdaterRollbackResult {
  /// Name of the bundle.
  pub name: String,
  /// Version requested by the rollback target.
  pub target_version: Option<String>,
  /// Version that was activated, when rollback succeeded.
  pub rollback_version: Option<String>,
  /// Rollback outcome.
  pub result: UpdaterRollbackResultKind,
}
