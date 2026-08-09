use crate::signature::SignatureVerifier;
use base64ct::{Base64, Encoding};
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey;

#[derive(Debug, Clone)]
pub struct EcdsaSecp256r1 {
  key: VerifyingKey,
}

impl EcdsaSecp256r1 {
  pub fn from_sec1_bytes(bytes: &[u8]) -> crate::Result<Self> {
    let key = VerifyingKey::from_sec1_bytes(bytes).map_err(crate::Error::invalid_signature_key)?;
    Ok(Self { key })
  }

  pub fn from_public_key_der(bytes: &[u8]) -> crate::Result<Self> {
    let key =
      VerifyingKey::from_public_key_der(bytes).map_err(crate::Error::invalid_signature_key)?;
    Ok(Self { key })
  }

  pub fn from_public_key_pem(pem: &str) -> crate::Result<Self> {
    let key =
      VerifyingKey::from_public_key_pem(pem).map_err(crate::Error::invalid_signature_key)?;
    Ok(Self { key })
  }
}

impl SignatureVerifier for EcdsaSecp256r1 {
  async fn verify(&self, data: &[u8], signature: &str) -> crate::Result<()> {
    let signature_bytes =
      Base64::decode_vec(signature).map_err(|_| crate::Error::InvalidSignature)?;
    let signature =
      Signature::from_slice(&signature_bytes).map_err(|_| crate::Error::InvalidSignature)?;
    self
      .key
      .verify(data, &signature)
      .map_err(|_| crate::Error::InvalidSignature)?;
    Ok(())
  }
}
