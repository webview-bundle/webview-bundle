use crate::signature::Verifier as SignatureVerifier;
use base64ct::{Base64, Encoding};
use ed25519_dalek::pkcs8::DecodePublicKey;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

pub struct Ed25519 {
  key: VerifyingKey,
}

impl Ed25519 {
  pub fn from_public_key_bytes(bytes: &[u8; 32]) -> crate::Result<Self> {
    let key = VerifyingKey::from_bytes(bytes).map_err(crate::Error::invalid_verifying_key)?;
    Ok(Self { key })
  }

  pub fn from_public_key_der(bytes: &[u8]) -> crate::Result<Self> {
    let key =
      VerifyingKey::from_public_key_der(bytes).map_err(crate::Error::invalid_verifying_key)?;
    Ok(Self { key })
  }

  pub fn from_public_key_pem(pem: &str) -> crate::Result<Self> {
    let key =
      VerifyingKey::from_public_key_pem(pem).map_err(crate::Error::invalid_verifying_key)?;
    Ok(Self { key })
  }
}

impl SignatureVerifier for Ed25519 {
  async fn verify(&self, data: &[u8], signature: &str) -> crate::Result<bool> {
    let signature_bytes =
      Base64::decode_vec(signature).map_err(|_| crate::Error::InvalidSignature)?;
    let signature =
      Signature::from_slice(&signature_bytes).map_err(|_| crate::Error::InvalidSignature)?;
    Ok(self.key.verify(data, &signature).is_ok())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use ed25519_dalek::{Signer, SigningKey};

  #[tokio::test]
  async fn verifies_base64_encoded_signature() {
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let message = b"sha256:n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg=";
    let signature_b64 = Base64::encode_string(&signing_key.sign(message).to_bytes());

    let verifier = Ed25519::from_public_key_bytes(&signing_key.verifying_key().to_bytes()).unwrap();

    assert!(verifier.verify(message, &signature_b64).await.unwrap());
    // Wrong message must not verify.
    assert!(!verifier.verify(b"tampered", &signature_b64).await.unwrap());
    // Raw (non-base64) signature bytes must fail to decode.
    assert!(verifier.verify(message, "!!!not base64!!!").await.is_err());
  }
}
