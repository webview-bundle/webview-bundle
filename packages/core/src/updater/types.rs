#[cfg(feature = "integrity")]
use crate::integrity;
use crate::remote::BundleUpdate;
#[cfg(feature = "signature")]
use crate::signature;
use crate::util::cancellation::Cancellation;

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
  pub key_sets: Option<Vec<signature::SignatureKeySet>>,
}

impl UpdaterSignatureOptions {
  pub fn key_set(self, key_set: signature::SignatureKeySet) -> Self {
    self.key_sets(vec![key_set])
  }

  pub fn key_sets(mut self, key_sets: Vec<signature::SignatureKeySet>) -> Self {
    self.key_sets = match self.key_sets {
      Some(original) => Some(vec![original, key_sets].concat()),
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdaterDownloadOptions {
  pub concurrency: Option<usize>,
  pub timeout: Option<u64>,
  pub cancellation: Option<Cancellation>,
}

#[derive(Debug)]
pub struct UpdaterDownloadResult {
  pub update: BundleUpdate,
  pub result: crate::Result<()>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdaterInstallBundleTarget {
  pub name: String,
  pub version: String,
  /// Bundles that must be installed together with this one. Targets are grouped transitively,
  /// so declaring the relation on one side is enough.
  pub atomic: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct UpdaterInstallResult {
  pub target: UpdaterInstallBundleTarget,
  pub result: crate::Result<()>,
}
