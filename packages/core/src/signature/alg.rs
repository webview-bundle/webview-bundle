#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureAlgorithm {
  RsaPkcs1V1_5Sha256,
  RsaPssSha256,
  EcdsaSecp256r1,
  EcdsaSecp384r1,
  Ed25519,
  Custom,
}

impl std::fmt::Display for SignatureAlgorithm {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let str = match self {
      Self::RsaPkcs1V1_5Sha256 => "rsa-pkcs1-v1_5-sha256",
      Self::RsaPssSha256 => "rsa-pss-sha256",
      Self::EcdsaSecp256r1 => "ecdsa-secp256r1",
      Self::EcdsaSecp384r1 => "ecdsa-secp384r1",
      Self::Ed25519 => "ed25519",
      Self::Custom => "custom",
    };
    write!(f, "{}", str)
  }
}
