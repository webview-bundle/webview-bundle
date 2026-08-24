use std::sync::Arc;
use wvb::signature;

#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait SignatureCustomVerify: Send + Sync {
  async fn verify(&self, message: Vec<u8>, signature: String) -> bool;
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct SignatureVerifyKey {
  pub id: String,
  pub verify: SignatureVerify,
}

#[derive(uniffi::Enum, Clone)]
pub enum SignatureVerify {
  EcdsaSecp256r1 {
    key: EcdsaVerifyingKey,
  },
  EcdsaSecp384r1 {
    key: EcdsaVerifyingKey,
  },
  Ed25519 {
    key: Ed25519VerifyingKey,
  },
  RsaPkcs1V15Sha256 {
    key: RsaVerifyingKey,
  },
  RsaPssSha256 {
    key: RsaVerifyingKey,
  },
  Custom {
    verify: Arc<dyn SignatureCustomVerify>,
  },
}

#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum EcdsaVerifyingKey {
  Sec1 { der: Vec<u8> },
  SpkiDer { der: Vec<u8> },
  SpkiPem { pem: String },
}

#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum Ed25519VerifyingKey {
  Raw { bytes: Vec<u8> },
  SpkiDer { der: Vec<u8> },
  SpkiPem { pem: String },
}

#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum RsaVerifyingKey {
  Pkcs1Der { der: Vec<u8> },
  Pkcs1Pem { pem: String },
  SpkiDer { der: Vec<u8> },
  SpkiPem { pem: String },
}

impl TryFrom<SignatureVerifyKey> for signature::SignatureVerifyKey {
  type Error = crate::Error;

  fn try_from(value: SignatureVerifyKey) -> Result<Self, Self::Error> {
    Ok(Self {
      id: value.id,
      verify: value.verify.try_into()?,
    })
  }
}

impl TryFrom<SignatureVerify> for signature::SignatureVerify {
  type Error = crate::Error;

  fn try_from(value: SignatureVerify) -> Result<Self, Self::Error> {
    let verify = match value {
      SignatureVerify::EcdsaSecp256r1 { key } => Self::EcdsaSecp256r1(match key {
        EcdsaVerifyingKey::Sec1 { der } => signature::EcdsaSecp256r1::from_sec1_bytes(&der)?,
        EcdsaVerifyingKey::SpkiDer { der } => signature::EcdsaSecp256r1::from_public_key_der(&der)?,
        EcdsaVerifyingKey::SpkiPem { pem } => signature::EcdsaSecp256r1::from_public_key_pem(&pem)?,
      }),
      SignatureVerify::EcdsaSecp384r1 { key } => Self::EcdsaSecp384r1(match key {
        EcdsaVerifyingKey::Sec1 { der } => signature::EcdsaSecp384r1::from_sec1_bytes(&der)?,
        EcdsaVerifyingKey::SpkiDer { der } => signature::EcdsaSecp384r1::from_public_key_der(&der)?,
        EcdsaVerifyingKey::SpkiPem { pem } => signature::EcdsaSecp384r1::from_public_key_pem(&pem)?,
      }),
      SignatureVerify::Ed25519 { key } => Self::Ed25519(match key {
        Ed25519VerifyingKey::Raw { bytes } => {
          let bytes: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
            crate::Error::invalid_signature_verify("ed25519 raw key must be 32 bytes")
          })?;
          signature::Ed25519::from_public_key_bytes(&bytes)?
        }
        Ed25519VerifyingKey::SpkiDer { der } => signature::Ed25519::from_public_key_der(&der)?,
        Ed25519VerifyingKey::SpkiPem { pem } => signature::Ed25519::from_public_key_pem(&pem)?,
      }),
      SignatureVerify::RsaPkcs1V15Sha256 { key } => Self::RsaPkcs1V1_5Sha256(match key {
        RsaVerifyingKey::Pkcs1Der { der } => signature::RsaPkcs1V15Sha256::from_pkcs1_der(&der)?,
        RsaVerifyingKey::Pkcs1Pem { pem } => signature::RsaPkcs1V15Sha256::from_pkcs1_pem(&pem)?,
        RsaVerifyingKey::SpkiDer { der } => {
          signature::RsaPkcs1V15Sha256::from_public_key_der(&der)?
        }
        RsaVerifyingKey::SpkiPem { pem } => {
          signature::RsaPkcs1V15Sha256::from_public_key_pem(&pem)?
        }
      }),
      SignatureVerify::RsaPssSha256 { key } => Self::RsaPssSha256(match key {
        RsaVerifyingKey::Pkcs1Der { der } => signature::RsaPssSha256::from_pkcs1_der(&der)?,
        RsaVerifyingKey::Pkcs1Pem { pem } => signature::RsaPssSha256::from_pkcs1_pem(&pem)?,
        RsaVerifyingKey::SpkiDer { der } => signature::RsaPssSha256::from_public_key_der(&der)?,
        RsaVerifyingKey::SpkiPem { pem } => signature::RsaPssSha256::from_public_key_pem(&pem)?,
      }),
      SignatureVerify::Custom { verify } => Self::Custom(into_custom_verify(verify)),
    };
    Ok(verify)
  }
}

impl std::fmt::Debug for SignatureVerify {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let name = match self {
      Self::EcdsaSecp256r1 { .. } => "EcdsaSecp256r1",
      Self::EcdsaSecp384r1 { .. } => "EcdsaSecp384r1",
      Self::Ed25519 { .. } => "Ed25519",
      Self::RsaPkcs1V15Sha256 { .. } => "RsaPkcs1V15Sha256",
      Self::RsaPssSha256 { .. } => "RsaPssSha256",
      Self::Custom { .. } => "Custom",
    };
    write!(f, "SignatureVerify::{name}")
  }
}

fn into_custom_verify(verify: Arc<dyn SignatureCustomVerify>) -> Arc<signature::CustomVerify> {
  Arc::new(move |message: &[u8], signature: &str| {
    let verify = verify.clone();
    let message = message.to_vec();
    let signature = signature.to_string();
    Box::pin(async move {
      Ok::<bool, Box<dyn std::error::Error + Send + Sync + 'static>>(
        verify.verify(message, signature).await,
      )
    })
  })
}
