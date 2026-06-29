use base64ct::{Base64, Encoding};
use sha2::{Digest, Sha256, Sha384, Sha512};
use std::str::FromStr;

#[derive(Default, Debug, Eq, PartialEq, Clone, Copy)]
pub enum IntegrityAlgorithm {
  #[default]
  Sha256,
  Sha384,
  Sha512,
}

impl IntegrityAlgorithm {
  pub fn digest(&self, data: &[u8]) -> Vec<u8> {
    match self {
      Self::Sha256 => Sha256::digest(data).to_vec(),
      Self::Sha384 => Sha384::digest(data).to_vec(),
      Self::Sha512 => Sha512::digest(data).to_vec(),
    }
  }
}

impl std::fmt::Display for IntegrityAlgorithm {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let str = match self {
      Self::Sha256 => "sha256",
      Self::Sha384 => "sha384",
      Self::Sha512 => "sha512",
    };
    write!(f, "{}", str)
  }
}

impl FromStr for IntegrityAlgorithm {
  type Err = crate::Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match &s.to_lowercase()[..] {
      "sha256" => Ok(Self::Sha256),
      "sha384" => Ok(Self::Sha384),
      "sha512" => Ok(Self::Sha512),
      _ => Err(crate::Error::invalid_integrity("invalid algorithm")),
    }
  }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Integrity {
  alg: IntegrityAlgorithm,
  value: Vec<u8>,
}

impl Integrity {
  pub fn compute(alg: IntegrityAlgorithm, data: &[u8]) -> Self {
    let value = alg.digest(data);
    Self { alg, value }
  }

  pub fn value(&self) -> &[u8] {
    &self.value
  }

  pub fn validate(&self, data: &[u8]) -> bool {
    self.value == self.alg.digest(data)
  }

  pub fn serialize(&self) -> String {
    let alg_str = self.alg.to_string();
    let encoded = Base64::encode_string(&self.value);
    format!("{alg_str}:{encoded}")
  }
}

impl FromStr for Integrity {
  type Err = crate::Error;

  fn from_str(str: &str) -> Result<Self, Self::Err> {
    let mut parts = str.splitn(2, ':');
    let alg_str = parts
      .next()
      .ok_or(crate::Error::invalid_integrity("algorithm is missing"))?;
    let alg = IntegrityAlgorithm::from_str(alg_str)?;
    let value_str = parts
      .next()
      .ok_or(crate::Error::invalid_integrity("value is missing"))?;
    let value = Base64::decode_vec(value_str)
      .map_err(|_| crate::Error::invalid_integrity("fail to decode value"))?;
    Ok(Self { alg, value })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn integrity_serialize() {
    let str = Integrity::compute(IntegrityAlgorithm::Sha256, b"test").serialize();
    assert_eq!(str, "sha256:n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg=");
  }

  #[test]
  fn integrity_from_str() {
    let str = "sha256:n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg=";
    let integrity = Integrity::from_str(str).unwrap();
    assert_eq!(integrity.alg, IntegrityAlgorithm::Sha256);
    assert_eq!(integrity.value, IntegrityAlgorithm::Sha256.digest(b"test"));
  }

  #[test]
  fn integrity_validate() {
    let integrity = Integrity::compute(IntegrityAlgorithm::Sha256, b"test");
    assert!(integrity.validate(b"test"));
    assert!(!integrity.validate(b"test2"));
  }
}
