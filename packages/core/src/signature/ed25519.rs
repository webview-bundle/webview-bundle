use crate::Bundle;
use crate::signature::Verifier as SignatureVerifier;
use ed25519_dalek::pkcs8::DecodePublicKey;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

pub struct Ed25519Verifier {
  key: VerifyingKey,
}

impl Ed25519Verifier {
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

impl SignatureVerifier for Ed25519Verifier {
  async fn verify(&self, _bundle: &Bundle, data: &[u8], signature: &str) -> crate::Result<bool> {
    let signature =
      Signature::from_slice(signature.as_bytes()).map_err(|_| crate::Error::InvalidSignature)?;
    Ok(self.key.verify(data, &signature).is_ok())
  }
}
