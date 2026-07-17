#[cfg(feature = "integrity")]
use crate::integrity::{IntegrityCheck, IntegrityPolicy};
#[cfg(feature = "signature")]
use crate::signature::SignatureVerify;
#[cfg(feature = "integrity")]
use crate::source::BundleSourceKind;
use crate::{DataReadOptions, HeaderReadOptions, IndexReadOptions};

/// Which bundles a load-time integrity verification applies to.
#[cfg(any(feature = "integrity", feature = "signature"))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BundleSourceVerifyMode {
  /// Verify both builtin and remote bundles.
  All,
  /// Verify downloaded (remote) bundles only.
  #[default]
  OnlyRemote,
}

#[cfg(any(feature = "integrity", feature = "signature"))]
impl BundleSourceVerifyMode {
  pub(crate) fn should_verify(&self, kind: &BundleSourceKind) -> bool {
    match self {
      Self::All => true,
      Self::OnlyRemote => *kind == BundleSourceKind::Remote,
    }
  }
}

/// How bundles are checked against the integrity recorded for them in the manifest when
/// they are loaded from disk.
#[cfg(feature = "integrity")]
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct BundleSourceIntegrityOptions {
  pub policy: IntegrityPolicy,
  pub check: IntegrityCheck,
  pub check_mode: BundleSourceVerifyMode,
}

#[cfg(feature = "integrity")]
impl BundleSourceIntegrityOptions {
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

  /// Which bundles are checked on load
  pub fn check_mode(mut self, mode: BundleSourceVerifyMode) -> Self {
    self.check_mode = mode;
    self
  }
}

/// How bundle signatures are verified when bundles are loaded from disk.
#[cfg(feature = "signature")]
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct BundleSourceSignatureOptions {
  pub verify: Option<SignatureVerify>,
  pub verify_mode: BundleSourceVerifyMode,
}

#[cfg(feature = "signature")]
impl BundleSourceSignatureOptions {
  /// Verifies that a bundle's integrity string was signed by the matching key
  pub fn verify(mut self, verifier: SignatureVerify) -> Self {
    self.verify = Some(verifier);
    self
  }

  /// Which bundles have their signature verified on load
  pub fn verify_mode(mut self, mode: BundleSourceVerifyMode) -> Self {
    self.verify_mode = mode;
    self
  }
}

#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct BundleSourceOptions {
  pub header_read: HeaderReadOptions,
  pub index_read: IndexReadOptions,
  pub data_read: DataReadOptions,
  #[cfg(feature = "integrity")]
  pub integrity: BundleSourceIntegrityOptions,
  #[cfg(feature = "signature")]
  pub signature: BundleSourceSignatureOptions,
}

impl BundleSourceOptions {
  /// How a bundle's header is checked when its descriptor is read on load.
  pub fn header_read(mut self, options: HeaderReadOptions) -> Self {
    self.header_read = options;
    self
  }

  /// How a bundle's index is checked when its descriptor is read on load.
  pub fn index_read(mut self, options: IndexReadOptions) -> Self {
    self.index_read = options;
    self
  }

  /// How entry data read through this source is checked
  pub fn data_read(mut self, options: DataReadOptions) -> Self {
    self.data_read = options;
    self
  }

  /// How bundles are checked against their manifest integrity metadata on load.
  #[cfg(feature = "integrity")]
  pub fn integrity(mut self, options: BundleSourceIntegrityOptions) -> Self {
    self.integrity = options;
    self
  }

  /// How bundle signatures are verified on load.
  #[cfg(feature = "signature")]
  pub fn signature(mut self, options: BundleSourceSignatureOptions) -> Self {
    self.signature = options;
    self
  }
}
