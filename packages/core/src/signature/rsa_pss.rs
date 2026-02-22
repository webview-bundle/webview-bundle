use crate::Bundle;
use crate::signature::Verifier as SignatureVerifier;
use rsa::RsaPublicKey;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::pss::{Signature, VerifyingKey};
use rsa::sha2::Sha256;
use rsa::signature::Verifier;

pub struct RsaPssVerifier {
  key: VerifyingKey<Sha256>,
}

impl RsaPssVerifier {
  pub fn from_public_key_der(bytes: &[u8]) -> crate::Result<Self> {
    let public_key =
      RsaPublicKey::from_public_key_der(bytes).map_err(crate::Error::invalid_verifying_key)?;
    let key = VerifyingKey::<Sha256>::from(public_key);
    Ok(Self { key })
  }

  pub fn from_public_key_pem(pem: &str) -> crate::Result<Self> {
    let public_key =
      RsaPublicKey::from_public_key_pem(pem).map_err(crate::Error::invalid_verifying_key)?;
    let key = VerifyingKey::<Sha256>::from(public_key);
    Ok(Self { key })
  }

  pub fn from_pkcs1_der(bytes: &[u8]) -> crate::Result<Self> {
    let public_key =
      RsaPublicKey::from_pkcs1_der(bytes).map_err(crate::Error::invalid_verifying_key)?;
    let key = VerifyingKey::<Sha256>::from(public_key);
    Ok(Self { key })
  }

  pub fn from_pkcs1_pem(pem: &str) -> crate::Result<Self> {
    let public_key =
      RsaPublicKey::from_pkcs1_pem(pem).map_err(crate::Error::invalid_verifying_key)?;
    let key = VerifyingKey::<Sha256>::from(public_key);
    Ok(Self { key })
  }
}

impl SignatureVerifier for RsaPssVerifier {
  async fn verify(&self, _bundle: &Bundle, data: &[u8], signature: &str) -> crate::Result<bool> {
    let signature =
      Signature::try_from(signature.as_bytes()).map_err(|_| crate::Error::InvalidSignature)?;
    Ok(self.key.verify(data, &signature).is_ok())
  }
}
