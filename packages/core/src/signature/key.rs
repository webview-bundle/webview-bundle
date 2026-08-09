use crate::signature::alg::SignatureAlgorithm;
use std::pin::Pin;
use std::sync::Arc;

/// Type alias for custom verification functions.
pub type CustomKey = dyn Fn(
    &[u8],
    &str,
  ) -> Pin<
    Box<
      dyn Future<Output = Result<bool, Box<dyn std::error::Error + Send + Sync + 'static>>>
        + Send
        + 'static,
    >,
  > + Send
  + Sync;

#[derive(Debug)]
pub struct SignatureKeySet {
  pub id: String,
  pub key: SignatureKey,
}

impl SignatureKeySet {
  pub fn algorithm(&self) -> SignatureAlgorithm {
    match &self.key {
      SignatureKey::RsaPkcs1V1_5Sha256(_) => SignatureAlgorithm::RsaPkcs1V1_5Sha256,
      SignatureKey::RsaPssSha256(_) => SignatureAlgorithm::RsaPssSha256,
      SignatureKey::EcdsaSecp256r1(_) => SignatureAlgorithm::EcdsaSecp256r1,
      SignatureKey::EcdsaSecp384r1(_) => SignatureAlgorithm::EcdsaSecp384r1,
      SignatureKey::Ed25519(_) => SignatureAlgorithm::Ed25519,
      SignatureKey::Custom(_) => SignatureAlgorithm::Custom,
    }
  }
}

pub enum SignatureKey {
  #[cfg(feature = "signature-rsa-pkcs1-v1_5-sha256")]
  RsaPkcs1V1_5Sha256(crate::signature::RsaPkcs1V15Sha256),
  #[cfg(feature = "signature-rsa-pss-sha256")]
  RsaPssSha256(crate::signature::RsaPssSha256),
  #[cfg(feature = "signature-ecdsa-secp256r1")]
  EcdsaSecp256r1(crate::signature::EcdsaSecp256r1),
  #[cfg(feature = "signature-ecdsa-secp384r1")]
  EcdsaSecp384r1(crate::signature::EcdsaSecp384r1),
  #[cfg(feature = "signature-ed25519")]
  Ed25519(crate::signature::Ed25519),
  Custom(Arc<CustomKey>),
}

impl std::fmt::Debug for SignatureKey {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let name = match self {
      #[cfg(feature = "signature-rsa-pkcs1-v1_5-sha256")]
      Self::RsaPkcs1V1_5Sha256(_) => "RsaPkcs1V15Sha256",
      #[cfg(feature = "signature-rsa-pss-sha256")]
      Self::RsaPssSha256(_) => "RsaPssSha256",
      #[cfg(feature = "signature-ecdsa-secp256r1")]
      Self::EcdsaSecp256r1(_) => "EcdsaSecp256r1",
      #[cfg(feature = "signature-ecdsa-secp384r1")]
      Self::EcdsaSecp384r1(_) => "EcdsaSecp384r1",
      #[cfg(feature = "signature-ed25519")]
      Self::Ed25519(_) => "Ed25519",
      Self::Custom(_) => "Custom",
    };
    write!(f, "SignatureKey::{name}")
  }
}
