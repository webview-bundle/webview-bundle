use crate::signature::SignatureVerifier;
use base64ct::{Base64, Encoding};
use rsa::RsaPublicKey;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs1v15::{Signature, VerifyingKey};
use rsa::pkcs8::DecodePublicKey;
use rsa::sha2::Sha256;
use rsa::signature::Verifier;

#[derive(Debug, Clone)]
pub struct RsaPkcs1V15Sha256 {
  key: VerifyingKey<Sha256>,
}

impl RsaPkcs1V15Sha256 {
  pub fn from_public_key_der(bytes: &[u8]) -> crate::Result<Self> {
    let public_key =
      RsaPublicKey::from_public_key_der(bytes).map_err(crate::Error::invalid_signature_key)?;
    let key = VerifyingKey::<Sha256>::from(public_key);
    Ok(Self { key })
  }

  pub fn from_public_key_pem(pem: &str) -> crate::Result<Self> {
    let public_key =
      RsaPublicKey::from_public_key_pem(pem).map_err(crate::Error::invalid_signature_key)?;
    let key = VerifyingKey::<Sha256>::from(public_key);
    Ok(Self { key })
  }

  pub fn from_pkcs1_der(bytes: &[u8]) -> crate::Result<Self> {
    let public_key =
      RsaPublicKey::from_pkcs1_der(bytes).map_err(crate::Error::invalid_signature_key)?;
    let key = VerifyingKey::<Sha256>::from(public_key);
    Ok(Self { key })
  }

  pub fn from_pkcs1_pem(pem: &str) -> crate::Result<Self> {
    let public_key =
      RsaPublicKey::from_pkcs1_pem(pem).map_err(crate::Error::invalid_signature_key)?;
    let key = VerifyingKey::<Sha256>::from(public_key);
    Ok(Self { key })
  }
}

impl SignatureVerifier for RsaPkcs1V15Sha256 {
  async fn verify(&self, data: &[u8], signature: &str) -> crate::Result<()> {
    let signature_bytes =
      Base64::decode_vec(signature).map_err(|_| crate::Error::InvalidSignature)?;
    let signature = Signature::try_from(signature_bytes.as_slice())
      .map_err(|_| crate::Error::InvalidSignature)?;
    self
      .key
      .verify(data, &signature)
      .map_err(|_| crate::Error::SignatureVerifyFailed)?;
    Ok(())
  }
}
