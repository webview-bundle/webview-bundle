use crate::Bundle;
use crate::signature::Verifier as SignatureVerifier;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey;

pub struct EcdsaSecp256r1Verifier {
  key: VerifyingKey,
}

impl EcdsaSecp256r1Verifier {
  pub fn from_sec1_bytes(bytes: &[u8]) -> crate::Result<Self> {
    let key = VerifyingKey::from_sec1_bytes(bytes).map_err(crate::Error::invalid_verifying_key)?;
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

impl SignatureVerifier for EcdsaSecp256r1Verifier {
  async fn verify(&self, _bundle: &Bundle, data: &[u8], signature: &str) -> crate::Result<bool> {
    let signature =
      Signature::from_slice(signature.as_bytes()).map_err(|_| crate::Error::InvalidSignature)?;
    Ok(self.key.verify(data, &signature).is_ok())
  }
}
