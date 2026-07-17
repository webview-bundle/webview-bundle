use crate::integrity::{IntegrityCheck, IntegrityPolicy};
use crate::signature::SignatureVerify;

/// How bundles are checked against the integrity recorded for them
/// in the remote manifest.
#[cfg(feature = "integrity")]
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct UpdaterIntegrityOptions {
  pub policy: IntegrityPolicy,
  pub check: IntegrityCheck,
}

#[cfg(feature = "integrity")]
impl UpdaterIntegrityOptions {
  /// How a bundle's integrity metadata is treated
  pub fn policy(mut self, policy: IntegrityPolicy) -> Self {
    self.policy = policy;
    self
  }

  /// The checker that validates bundle bytes against an integrity string
  pub fn check(mut self, check: IntegrityCheck) -> Self {
    self.check = check;
    self
  }
}

/// How bundle signatures are verified when bundles are loaded from remote.
#[cfg(feature = "signature")]
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct UpdaterSignatureOptions {
  pub verify: Option<SignatureVerify>,
}

impl UpdaterSignatureOptions {
  /// Verifies that a bundle's integrity string was signed by the matching key
  pub fn verify(mut self, verify: SignatureVerify) -> Self {
    self.verify = Some(verify);
    self
  }
}

#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct UpdaterOptions {
  pub channel: Option<String>,
  #[cfg(feature = "integrity")]
  pub integrity: UpdaterIntegrityOptions,
  #[cfg(feature = "signature")]
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
