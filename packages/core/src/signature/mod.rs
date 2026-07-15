//! Digital signature verification for bundles.
//!
//! The signature module provides cryptographic signature verification to ensure
//! bundles are authentic and haven't been tampered with. Multiple signature
//! algorithms are supported:
//!
//! - **ECDSA** (secp256r1, secp384r1) - Elliptic curve signatures
//! - **Ed25519** - Edwards curve signatures
//! - **RSA** (PKCS#1 v1.5, PSS) - RSA signatures
//!
//! ## Features
//!
//! Enable specific signature algorithms via cargo features:
//!
//! - `signature-ecdsa_secp256r1` - ECDSA with P-256 curve
//! - `signature-ecdsa_secp384r1` - ECDSA with P-384 curve
//! - `signature-edd25519` - Ed25519 signatures
//! - `signature-rsa_pkcs1_v1_5` - RSA PKCS#1 v1.5
//! - `signature-rsa_pss` - RSA-PSS
//!
//! ## Example
//!
//! ```no_run
//! # #[cfg(feature = "signature-edd25519")]
//! # async {
//! use wvb::signature::{Ed25519Verifier, SignatureVerifier};
//! use std::sync::Arc;
//!
//! // Create verifier with public key PEM
//! let public_key_pem = "-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----";
//! let verifier = Ed25519Verifier::from_public_key_pem(public_key_pem).unwrap();
//! let signature_verifier = SignatureVerifier::Ed25519(Arc::new(verifier));
//!
//! // The signed message is the bundle's integrity string, not the bundle bytes:
//! // the signature authenticates the integrity string, and the integrity string
//! // authenticates the bytes. Both checks must run for either to mean anything.
//! let message = b"sha256:n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg=";
//! let signature = "base64-encoded-signature";
//!
//! let is_valid = signature_verifier.verify(message, signature).await.unwrap();
//! assert!(is_valid);
//! # };
//! ```
//!
//! ## Custom Verifiers
//!
//! Implement custom verification logic:
//!
//! ```no_run
//! # use wvb::signature::SignatureVerifier;
//! # use std::sync::Arc;
//! let custom_verify = |message: &[u8], signature: &str| {
//!     Box::pin(async move {
//!         // Custom verification logic
//!         Ok(true)
//!     })
//! };
//!
//! let verifier = SignatureVerifier::Custom(Arc::new(custom_verify));
//! ```

use std::pin::Pin;
use std::sync::Arc;

#[cfg(feature = "signature-ecdsa_secp256r1")]
mod ecdsa_secp256r1;
#[cfg(feature = "signature-ecdsa_secp384r1")]
mod ecdsa_secp384r1;
#[cfg(feature = "signature-edd25519")]
mod ed25519;
#[cfg(feature = "signature-rsa_pkcs1_v1_5")]
mod rsa_pkcs1_v1_5;
#[cfg(feature = "signature-rsa_pss")]
mod rsa_pss;
#[cfg(feature = "signature-ecdsa_secp256r1")]
pub use ecdsa_secp256r1::*;
#[cfg(feature = "signature-ecdsa_secp384r1")]
pub use ecdsa_secp384r1::*;
#[cfg(feature = "signature-edd25519")]
pub use ed25519::*;
#[cfg(feature = "signature-rsa_pkcs1_v1_5")]
pub use rsa_pkcs1_v1_5::*;
#[cfg(feature = "signature-rsa_pss")]
pub use rsa_pss::*;

/// Type alias for custom verification functions.
///
/// Custom verifiers receive the signed message and the signature, and return
/// a future that resolves to the verification result.
pub type CustomVerify = dyn Fn(
    &[u8],
    &str,
  ) -> Pin<
    Box<
      dyn std::future::Future<
          Output = Result<bool, Box<dyn std::error::Error + Send + Sync + 'static>>,
        > + Send
        + 'static,
    >,
  > + Send
  + Sync;

/// Signature verifier supporting multiple algorithms.
///
/// This enum wraps different signature verification implementations,
/// allowing you to use the appropriate algorithm for your needs.
#[non_exhaustive]
#[derive(Clone)]
pub enum SignatureVerifier {
  #[cfg(feature = "signature-ecdsa_secp256r1")]
  EcdsaSecp256r1(Arc<EcdsaSecp256r1Verifier>),
  #[cfg(feature = "signature-ecdsa_secp384r1")]
  EcdsaSecp384r1(Arc<EcdsaSecp384r1Verifier>),
  #[cfg(feature = "signature-edd25519")]
  Ed25519(Arc<Ed25519Verifier>),
  #[cfg(feature = "signature-rsa_pkcs1_v1_5")]
  RsaPkcs1V15(Arc<RsaPkcs1V15Verifier>),
  #[cfg(feature = "signature-rsa_pss")]
  RsaPss(Arc<RsaPssVerifier>),
  Custom(Arc<CustomVerify>),
}

impl std::fmt::Debug for SignatureVerifier {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let name = match self {
      Self::Custom(_) => "Custom",
      #[cfg(feature = "signature-ecdsa_secp256r1")]
      Self::EcdsaSecp256r1(_) => "EcdsaSecp256r1",
      #[cfg(feature = "signature-ecdsa_secp384r1")]
      Self::EcdsaSecp384r1(_) => "EcdsaSecp384r1",
      #[cfg(feature = "signature-edd25519")]
      Self::Ed25519(_) => "Ed25519",
      #[cfg(feature = "signature-rsa_pkcs1_v1_5")]
      Self::RsaPkcs1V15(_) => "RsaPkcs1V15",
      #[cfg(feature = "signature-rsa_pss")]
      Self::RsaPss(_) => "RsaPss",
    };
    write!(f, "SignatureVerifier::{name}")
  }
}

impl SignatureVerifier {
  /// Verifies a signature against a message using the configured algorithm.
  ///
  /// # Arguments
  ///
  /// * `message` - The message that was signed. For bundles this is the integrity
  ///   string (e.g. `sha256:<base64>`), so the signature authenticates the integrity
  ///   string and the integrity string authenticates the bundle bytes.
  /// * `signature` - The signature string (base64-encoded)
  ///
  /// # Returns
  ///
  /// Returns `Ok(true)` if the signature is valid, `Ok(false)` if invalid,
  /// or an error if verification failed.
  pub async fn verify(&self, message: &[u8], signature: &str) -> crate::Result<bool> {
    match self {
      Self::Custom(verify) => verify(message, signature)
        .await
        .map_err(crate::Error::generic),
      #[cfg(feature = "signature-ecdsa_secp256r1")]
      Self::EcdsaSecp256r1(verifier) => verifier.verify(message, signature).await,
      #[cfg(feature = "signature-ecdsa_secp384r1")]
      Self::EcdsaSecp384r1(verifier) => verifier.verify(message, signature).await,
      #[cfg(feature = "signature-edd25519")]
      Self::Ed25519(verifier) => verifier.verify(message, signature).await,
      #[cfg(feature = "signature-rsa_pkcs1_v1_5")]
      Self::RsaPkcs1V15(verifier) => verifier.verify(message, signature).await,
      #[cfg(feature = "signature-rsa_pss")]
      Self::RsaPss(verifier) => verifier.verify(message, signature).await,
    }
  }
}

/// Verifies that `signature` is a valid signature over the `integrity` string.
///
/// The signature signs the integrity string (e.g. `sha256:<base64>`), not the bundle
/// bytes. A verifier with no integrity string to verify against is a misconfiguration
/// ([`crate::Error::IntegrityRequired`]); a missing signature fails with
/// [`crate::Error::SignatureNotExists`].
pub(crate) async fn verify_signature(
  verifier: &SignatureVerifier,
  integrity: Option<&str>,
  signature: Option<&str>,
) -> crate::Result<()> {
  let message = integrity.ok_or(crate::Error::IntegrityRequired)?;
  let signature = signature.ok_or(crate::Error::SignatureNotExists)?;
  if !verifier.verify(message.as_bytes(), signature).await? {
    return Err(crate::Error::SignatureVerifyFailed);
  }
  Ok(())
}

/// Trait for implementing signature verification algorithms.
///
/// Implement this trait to create custom signature verifiers that can be
/// used with the `SignatureVerifier::Custom` variant.
pub trait Verifier: Send + Sync + 'static {
  /// Verifies a signature.
  ///
  /// # Arguments
  ///
  /// * `message` - The signed message data
  /// * `signature` - The signature string to verify
  fn verify(
    &self,
    message: &[u8],
    signature: &str,
  ) -> impl std::future::Future<Output = crate::Result<bool>>;
}

#[cfg(all(test, feature = "signature-edd25519"))]
mod tests {
  use super::*;
  use base64ct::{Base64, Encoding};
  use ed25519_dalek::{Signer, SigningKey};

  fn verifier_and_sign(message: &str) -> (SignatureVerifier, String) {
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let signature = Base64::encode_string(&signing_key.sign(message.as_bytes()).to_bytes());
    let verifier =
      Ed25519Verifier::from_public_key_bytes(&signing_key.verifying_key().to_bytes()).unwrap();
    (SignatureVerifier::Ed25519(Arc::new(verifier)), signature)
  }

  #[tokio::test]
  async fn verifies_over_the_integrity_string() {
    let integrity = "sha256:n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg=";
    let (verifier, signature) = verifier_and_sign(integrity);
    verify_signature(&verifier, Some(integrity), Some(&signature))
      .await
      .unwrap();
  }

  #[tokio::test]
  async fn verifier_without_integrity_is_a_misconfiguration() {
    let (verifier, signature) = verifier_and_sign("sha256:whatever");
    let err = verify_signature(&verifier, None, Some(&signature))
      .await
      .unwrap_err();
    assert!(matches!(err, crate::Error::IntegrityRequired));
  }

  #[tokio::test]
  async fn missing_signature_fails() {
    let (verifier, _) = verifier_and_sign("sha256:whatever");
    let err = verify_signature(&verifier, Some("sha256:whatever"), None)
      .await
      .unwrap_err();
    assert!(matches!(err, crate::Error::SignatureNotExists));
  }

  #[tokio::test]
  async fn wrong_signature_fails() {
    let (verifier, signature) = verifier_and_sign("sha256:a-different-message");
    let err = verify_signature(
      &verifier,
      Some("sha256:the-actual-message"),
      Some(&signature),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, crate::Error::SignatureVerifyFailed));
  }
}
