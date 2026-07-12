//! Integrity and signature verification of raw bundle bytes.
//!
//! Shared by [`crate::source::BundleSource`] (verify when a bundle is loaded from disk)
//! and [`crate::updater::Updater`] (verify when a bundle is downloaded or installed), so
//! both apply the same rules to the same bytes.
//!
//! ## What is actually signed
//!
//! A bundle's signature signs its **integrity string** (e.g. `sha256:<base64>`), not the
//! bundle bytes. The trust chain is therefore two links long:
//!
//! 1. the signature authenticates the integrity string, and
//! 2. the integrity string authenticates the bytes.
//!
//! Checking only the signature proves nothing about the bytes, so [`VerifyOptions::verify`]
//! always runs the integrity check when a signature verifier is configured, whatever the
//! [`IntegrityPolicy`] says.

use crate::integrity::{IntegrityChecker, IntegrityPolicy};
#[cfg(feature = "signature")]
use crate::signature::SignatureVerifier;

/// How bundle bytes are verified against their advertised integrity and signature.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct VerifyOptions {
  pub(crate) integrity_policy: IntegrityPolicy,
  pub(crate) integrity_checker: IntegrityChecker,
  #[cfg(feature = "signature")]
  pub(crate) signature_verifier: Option<SignatureVerifier>,
}

impl VerifyOptions {
  pub(crate) fn set_integrity_policy(&mut self, policy: IntegrityPolicy) {
    self.integrity_policy = policy;
  }

  pub(crate) fn set_integrity_checker(&mut self, checker: IntegrityChecker) {
    self.integrity_checker = checker;
  }

  #[cfg(feature = "signature")]
  pub(crate) fn set_signature_verifier(&mut self, verifier: SignatureVerifier) {
    self.signature_verifier = Some(verifier);
  }

  /// Whether a signature verifier is configured. When one is, the integrity string is the
  /// signed message and so becomes mandatory.
  fn signature_configured(&self) -> bool {
    #[cfg(feature = "signature")]
    {
      self.signature_verifier.is_some()
    }
    #[cfg(not(feature = "signature"))]
    {
      false
    }
  }

  /// Verifies `data` against the `integrity` and `signature` advertised for it.
  ///
  /// `integrity` and `signature` come from the remote's response headers (on download) or
  /// from the bundle manifest (on load/install).
  pub(crate) async fn verify(
    &self,
    integrity: Option<&str>,
    signature: Option<&str>,
    data: &[u8],
  ) -> crate::Result<()> {
    let signature_configured = self.signature_configured();

    if signature_configured || self.integrity_policy != IntegrityPolicy::None {
      match integrity {
        Some(integrity) => self.integrity_checker.check(integrity, data).await?,
        // A verifier with nothing to verify against is a misconfiguration, not a pass.
        None if signature_configured => return Err(crate::Error::IntegrityRequired),
        None if self.integrity_policy == IntegrityPolicy::Strict => {
          return Err(crate::Error::IntegrityVerifyFailed);
        }
        None => {}
      }
    }

    #[cfg(feature = "signature")]
    if let Some(verifier) = &self.signature_verifier {
      // Checked above: `signature_configured` forces `integrity` to be present.
      let message = integrity.ok_or(crate::Error::IntegrityRequired)?;
      let signature = signature.ok_or(crate::Error::SignatureNotExists)?;
      if !verifier.verify(message.as_bytes(), signature).await? {
        return Err(crate::Error::SignatureVerifyFailed);
      }
    }
    #[cfg(not(feature = "signature"))]
    let _ = signature;

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::integrity::{Integrity, IntegrityAlgorithm};

  const DATA: &[u8] = b"bundle bytes";

  fn integrity_of(data: &[u8]) -> String {
    Integrity::compute(IntegrityAlgorithm::Sha256, data).serialize()
  }

  #[tokio::test]
  async fn policy_none_skips_integrity() {
    let mut options = VerifyOptions::default();
    options.set_integrity_policy(IntegrityPolicy::None);
    // Even a wrong integrity string is not looked at.
    let wrong = integrity_of(b"other bytes");
    options.verify(Some(&wrong), None, DATA).await.unwrap();
  }

  #[tokio::test]
  async fn policy_optional_checks_when_present() {
    let mut options = VerifyOptions::default();
    options.set_integrity_policy(IntegrityPolicy::Optional);
    options
      .verify(Some(&integrity_of(DATA)), None, DATA)
      .await
      .unwrap();
    // Absent integrity is tolerated.
    options.verify(None, None, DATA).await.unwrap();
    // Present but wrong is not.
    let err = options
      .verify(Some(&integrity_of(b"other")), None, DATA)
      .await
      .unwrap_err();
    assert!(matches!(err, crate::Error::IntegrityVerifyFailed));
  }

  #[tokio::test]
  async fn policy_strict_requires_integrity() {
    let mut options = VerifyOptions::default();
    options.set_integrity_policy(IntegrityPolicy::Strict);
    let err = options.verify(None, None, DATA).await.unwrap_err();
    assert!(matches!(err, crate::Error::IntegrityVerifyFailed));
  }

  #[cfg(feature = "signature-edd25519")]
  mod signature {
    use super::*;
    use crate::signature::Ed25519Verifier;
    use base64ct::{Base64, Encoding};
    use ed25519_dalek::{Signer, SigningKey};
    use std::sync::Arc;

    fn verifier_and_sign(message: &str) -> (SignatureVerifier, String) {
      let signing_key = SigningKey::from_bytes(&[7u8; 32]);
      let signature = Base64::encode_string(&signing_key.sign(message.as_bytes()).to_bytes());
      let verifier =
        Ed25519Verifier::from_public_key_bytes(&signing_key.verifying_key().to_bytes()).unwrap();
      (SignatureVerifier::Ed25519(Arc::new(verifier)), signature)
    }

    #[tokio::test]
    async fn signature_verifies_over_the_integrity_string() {
      let integrity = integrity_of(DATA);
      let (verifier, signature) = verifier_and_sign(&integrity);
      let mut options = VerifyOptions::default();
      options.set_signature_verifier(verifier);
      options
        .verify(Some(&integrity), Some(&signature), DATA)
        .await
        .unwrap();
    }

    /// A signature over an integrity string that does not match the data must fail, even
    /// though the signature itself is valid — this is the hole the shared helper closes.
    #[tokio::test]
    async fn valid_signature_over_mismatched_integrity_fails() {
      let integrity = integrity_of(b"the bytes that were signed");
      let (verifier, signature) = verifier_and_sign(&integrity);
      let mut options = VerifyOptions::default();
      // Even with the policy explicitly disabled, a configured verifier forces the check.
      options.set_integrity_policy(IntegrityPolicy::None);
      options.set_signature_verifier(verifier);
      let err = options
        .verify(Some(&integrity), Some(&signature), DATA)
        .await
        .unwrap_err();
      assert!(matches!(err, crate::Error::IntegrityVerifyFailed));
    }

    #[tokio::test]
    async fn verifier_without_integrity_is_a_misconfiguration() {
      let (verifier, signature) = verifier_and_sign("sha256:whatever");
      let mut options = VerifyOptions::default();
      options.set_signature_verifier(verifier);
      let err = options
        .verify(None, Some(&signature), DATA)
        .await
        .unwrap_err();
      assert!(matches!(err, crate::Error::IntegrityRequired));
    }

    #[tokio::test]
    async fn missing_signature_fails() {
      let integrity = integrity_of(DATA);
      let (verifier, _) = verifier_and_sign(&integrity);
      let mut options = VerifyOptions::default();
      options.set_signature_verifier(verifier);
      let err = options
        .verify(Some(&integrity), None, DATA)
        .await
        .unwrap_err();
      assert!(matches!(err, crate::Error::SignatureNotExists));
    }

    #[tokio::test]
    async fn wrong_signature_fails() {
      let integrity = integrity_of(DATA);
      let (verifier, _) = verifier_and_sign("sha256:a-different-message");
      let (_, signature) = verifier_and_sign("sha256:a-different-message");
      let mut options = VerifyOptions::default();
      options.set_signature_verifier(verifier);
      let err = options
        .verify(Some(&integrity), Some(&signature), DATA)
        .await
        .unwrap_err();
      assert!(matches!(err, crate::Error::SignatureVerifyFailed));
    }
  }
}
