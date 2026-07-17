use std::str::FromStr;
use std::sync::Arc;
use wvb::integrity;

/// A custom checker that validates a bundle's bytes against its integrity string.
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait IntegrityCheck: Send + Sync {
  async fn check(&self, data: Vec<u8>, integrity: String) -> bool;
}

pub(crate) fn into_checker(check: Arc<dyn IntegrityCheck>) -> integrity::IntegrityCheck {
  integrity::IntegrityCheck::Custom(Arc::new(move |data: &[u8], integrity: &str| {
    let check = check.clone();
    let data = data.to_vec();
    let integrity = integrity.to_string();
    Box::pin(async move {
      Ok::<bool, Box<dyn std::error::Error + Send + Sync + 'static>>(
        check.check(data, integrity).await,
      )
    })
  }))
}

/// Hash algorithm used to compute the [`Subresource Integrity`](https://developer.mozilla.org/en-US/docs/Web/Security/Subresource_Integrity) digest of a bundle.
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

/// A digest over some bytes, serialized as `<algorithm>:<base64>` (e.g. `"sha256:n4bQ..."`).
#[derive(uniffi::Object)]
pub struct Integrity {
  inner: integrity::Integrity,
}

#[uniffi::export]
impl Integrity {
  /// Computes the integrity of `data` with `algorithm`.
  #[uniffi::constructor(name = "compute")]
  pub fn compute(algorithm: IntegrityAlgorithm, data: Vec<u8>) -> Arc<Integrity> {
    Arc::new(Integrity {
      inner: integrity::Integrity::compute(algorithm.into(), &data),
    })
  }

  /// Parses a serialized integrity string (e.g. `"sha256:n4bQ..."`).
  #[uniffi::constructor(name = "parse")]
  pub fn parse(integrity: String) -> Result<Arc<Integrity>, crate::Error> {
    Ok(Arc::new(Integrity {
      inner: integrity::Integrity::from_str(&integrity)?,
    }))
  }

  /// The raw digest bytes.
  pub fn value(&self) -> Vec<u8> {
    self.inner.value().to_vec()
  }

  /// Whether `data` digests to this integrity.
  pub fn validate(&self, data: Vec<u8>) -> bool {
    self.inner.validate(&data)
  }

  /// Serializes to `<algorithm>:<base64>`.
  pub fn serialize(&self) -> String {
    self.inner.serialize()
  }
}

/// How a bundle's integrity metadata is treated when the integrity check runs.
///
/// - `Strict`: integrity metadata is required; a bundle without it fails the check.
/// - `Optional`: integrity metadata is checked when present and tolerated when missing.
/// - `Off`: the integrity check is disabled.
#[derive(uniffi::Enum, Clone, Debug)]
pub enum IntegrityPolicy {
  Strict,
  Optional,
  Off,
}

impl From<IntegrityPolicy> for integrity::IntegrityPolicy {
  fn from(v: IntegrityPolicy) -> Self {
    match v {
      IntegrityPolicy::Strict => integrity::IntegrityPolicy::Strict,
      IntegrityPolicy::Optional => integrity::IntegrityPolicy::Optional,
      IntegrityPolicy::Off => integrity::IntegrityPolicy::Off,
    }
  }
}

impl From<integrity::IntegrityPolicy> for IntegrityPolicy {
  fn from(v: integrity::IntegrityPolicy) -> Self {
    match v {
      integrity::IntegrityPolicy::Strict => IntegrityPolicy::Strict,
      integrity::IntegrityPolicy::Optional => IntegrityPolicy::Optional,
      integrity::IntegrityPolicy::Off => IntegrityPolicy::Off,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn compute_produces_the_string_core_produces() {
    // Same vector as core's own integrity_serialize test.
    let integrity = Integrity::compute(IntegrityAlgorithm::Sha256, b"test".to_vec());
    assert_eq!(
      integrity.serialize(),
      "sha256:n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg="
    );
  }

  #[test]
  fn parse_round_trips_and_validates_the_right_bytes_only() {
    let data = b"<h1>hello</h1>".to_vec();
    let serialized = Integrity::compute(IntegrityAlgorithm::Sha384, data.clone()).serialize();
    let parsed = Integrity::parse(serialized.clone()).unwrap();

    assert_eq!(parsed.serialize(), serialized);
    assert_eq!(parsed.value().len(), 48); // sha384 = 48 bytes
    assert!(parsed.validate(data));
    assert!(!parsed.validate(b"tampered".to_vec()));
  }

  #[test]
  fn parse_rejects_a_malformed_string() {
    assert!(matches!(
      Integrity::parse("not-an-integrity".to_string()),
      Err(crate::Error::CoreInvalidIntegrity(_))
    ));
  }
}
