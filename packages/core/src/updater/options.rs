#[cfg(feature = "integrity")]
use crate::integrity;
#[cfg(feature = "signature")]
use crate::signature;

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
