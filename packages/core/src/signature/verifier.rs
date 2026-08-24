use crate::signature::verify::SignatureVerify;

/// Trait for implementing signature verification algorithms.
///
/// Implement this trait to create custom signature verifiers that can be
/// used with the `SignatureKey::Custom` variant.
pub trait SignatureVerifier: Send + Sync + 'static {
  /// Verifies a signature.
  ///
  /// # Arguments
  ///
  /// * `message` - The signed message data
  /// * `signature` - The signature string to verify
  fn verify(&self, message: &[u8], signature: &str) -> impl Future<Output = crate::Result<()>>;
}

impl SignatureVerifier for SignatureVerify {
  async fn verify(&self, message: &[u8], signature: &str) -> crate::Result<()> {
    match self {
      #[cfg(feature = "signature-rsa-pkcs1-v1_5-sha256")]
      Self::RsaPkcs1V1_5Sha256(key) => key.verify(message, signature).await,
      #[cfg(feature = "signature-rsa-pss-sha256")]
      Self::RsaPssSha256(key) => key.verify(message, signature).await,
      #[cfg(feature = "signature-ecdsa-secp256r1")]
      Self::EcdsaSecp256r1(key) => key.verify(message, signature).await,
      #[cfg(feature = "signature-ecdsa-secp384r1")]
      Self::EcdsaSecp384r1(key) => key.verify(message, signature).await,
      #[cfg(feature = "signature-ed25519")]
      Self::Ed25519(key) => key.verify(message, signature).await,
      Self::Custom(verify) => {
        match verify(message, signature)
          .await
          .map_err(crate::Error::generic)?
        {
          true => Ok(()),
          false => Err(crate::Error::SignatureVerifyFailed),
        }
      }
    }
  }
}

#[cfg(all(test, feature = "signature-ed25519"))]
mod tests {
  use super::*;
  use crate::signature::Ed25519;
  use base64ct::{Base64, Encoding};
  use ed25519_dalek::{Signer, SigningKey};

  fn verifier_and_sign(message: &str) -> (SignatureVerify, String) {
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let signature = Base64::encode_string(&signing_key.sign(message.as_bytes()).to_bytes());
    let ed25519 = Ed25519::from_public_key_bytes(&signing_key.verifying_key().to_bytes()).unwrap();
    (SignatureVerify::Ed25519(ed25519), signature)
  }

  #[tokio::test]
  async fn verify() {
    let message = "this_is_message";
    let (key, signature) = verifier_and_sign(message);
    assert!(key.verify(message.as_bytes(), &signature).await.is_ok());
  }

  #[tokio::test]
  async fn verify_failed() {
    let (key, signature) = verifier_and_sign("original_message");
    assert_eq!(
      key
        .verify("different_message".as_bytes(), &signature)
        .await
        .unwrap_err()
        .code(),
      crate::ErrorCode::SignatureVerifyFailed,
    );
  }
}
