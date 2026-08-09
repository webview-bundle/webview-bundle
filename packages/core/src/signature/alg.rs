#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SignatureAlgorithm {
  #[cfg_attr(feature = "_serde", serde(rename = "rsa-pkcs1-v1_5-sha256"))]
  RsaPkcs1V1_5Sha256,
  #[cfg_attr(feature = "_serde", serde(rename = "rsa-pss-sha256"))]
  RsaPssSha256,
  #[cfg_attr(feature = "_serde", serde(rename = "ecdsa-secp256r1"))]
  EcdsaSecp256r1,
  #[cfg_attr(feature = "_serde", serde(rename = "ecdsa-secp384r1"))]
  EcdsaSecp384r1,
  #[cfg_attr(feature = "_serde", serde(rename = "ed25519"))]
  Ed25519,
  #[cfg_attr(feature = "_serde", serde(rename = "custom"))]
  Custom,
}
