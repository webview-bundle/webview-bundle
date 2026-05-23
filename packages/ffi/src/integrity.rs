use wvb::integrity;

#[derive(uniffi::Enum, Clone, Debug)]
pub enum IntegrityAlgorithm {
  Sha256,
  Sha384,
  Sha512,
}

impl From<IntegrityAlgorithm> for integrity::IntegrityAlgorithm {
  fn from(v: IntegrityAlgorithm) -> Self {
    match v {
      IntegrityAlgorithm::Sha256 => integrity::IntegrityAlgorithm::Sha256,
      IntegrityAlgorithm::Sha384 => integrity::IntegrityAlgorithm::Sha384,
      IntegrityAlgorithm::Sha512 => integrity::IntegrityAlgorithm::Sha512,
    }
  }
}

impl From<integrity::IntegrityAlgorithm> for IntegrityAlgorithm {
  fn from(v: integrity::IntegrityAlgorithm) -> Self {
    match v {
      integrity::IntegrityAlgorithm::Sha256 => IntegrityAlgorithm::Sha256,
      integrity::IntegrityAlgorithm::Sha384 => IntegrityAlgorithm::Sha384,
      integrity::IntegrityAlgorithm::Sha512 => IntegrityAlgorithm::Sha512,
    }
  }
}

#[derive(uniffi::Enum, Clone, Debug)]
pub enum IntegrityPolicy {
  Strict,
  Optional,
  None,
}

impl From<IntegrityPolicy> for integrity::IntegrityPolicy {
  fn from(v: IntegrityPolicy) -> Self {
    match v {
      IntegrityPolicy::Strict => integrity::IntegrityPolicy::Strict,
      IntegrityPolicy::Optional => integrity::IntegrityPolicy::Optional,
      IntegrityPolicy::None => integrity::IntegrityPolicy::None,
    }
  }
}

impl From<integrity::IntegrityPolicy> for IntegrityPolicy {
  fn from(v: integrity::IntegrityPolicy) -> Self {
    match v {
      integrity::IntegrityPolicy::Strict => IntegrityPolicy::Strict,
      integrity::IntegrityPolicy::Optional => IntegrityPolicy::Optional,
      integrity::IntegrityPolicy::None => IntegrityPolicy::None,
    }
  }
}
