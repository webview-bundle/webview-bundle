use crate::integrity::{Integrity, IntegrityPolicy};
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;

pub type CustomChecker = dyn Fn(
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

#[non_exhaustive]
#[derive(Default, Clone)]
pub enum IntegrityChecker {
  #[default]
  Default,
  Custom(Arc<CustomChecker>),
}

impl std::fmt::Debug for IntegrityChecker {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let name = match self {
      Self::Default => "Default",
      Self::Custom(_) => "Custom",
    };
    write!(f, "IntegrityChecker::{name}")
  }
}

impl IntegrityChecker {
  pub async fn check(&self, integrity: &str, data: &[u8]) -> crate::Result<()> {
    match self {
      Self::Default => {
        let integrity = Integrity::from_str(integrity)?;
        if !integrity.validate(data) {
          return Err(crate::Error::IntegrityVerifyFailed);
        }
        Ok(())
      }
      Self::Custom(checker) => {
        if !checker(data, integrity)
          .await
          .map_err(crate::Error::generic)?
        {
          return Err(crate::Error::IntegrityVerifyFailed);
        }
        Ok(())
      }
    }
  }
}

/// Verifies `data` against the `integrity` advertised for it, as `policy` dictates.
///
/// `integrity` comes from the remote's response headers (on download) or from the bundle
/// manifest (on load/install).
pub(crate) async fn verify_integrity(
  policy: IntegrityPolicy,
  check: &IntegrityChecker,
  integrity: Option<&str>,
  data: &[u8],
) -> crate::Result<()> {
  if policy == IntegrityPolicy::Off {
    return Ok(());
  }
  match integrity {
    Some(integrity) => check.check(integrity, data).await,
    None if policy == IntegrityPolicy::Strict => Err(crate::Error::IntegrityVerifyFailed),
    None => Ok(()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::integrity::IntegrityAlgorithm;

  const DATA: &[u8] = b"bundle bytes";

  fn integrity_of(data: &[u8]) -> String {
    Integrity::compute(IntegrityAlgorithm::Sha256, data).serialize()
  }

  #[tokio::test]
  async fn policy_off_skips_the_check() {
    // Even a wrong integrity string is not looked at.
    let wrong = integrity_of(b"other bytes");
    verify_integrity(
      IntegrityPolicy::Off,
      &IntegrityChecker::Default,
      Some(&wrong),
      DATA,
    )
    .await
    .unwrap();
  }

  #[tokio::test]
  async fn policy_optional_checks_when_present() {
    let check = IntegrityChecker::Default;
    verify_integrity(
      IntegrityPolicy::Optional,
      &check,
      Some(&integrity_of(DATA)),
      DATA,
    )
    .await
    .unwrap();
    // Absent integrity is tolerated.
    verify_integrity(IntegrityPolicy::Optional, &check, None, DATA)
      .await
      .unwrap();
    // Present but wrong is not.
    let err = verify_integrity(
      IntegrityPolicy::Optional,
      &check,
      Some(&integrity_of(b"other")),
      DATA,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, crate::Error::IntegrityVerifyFailed));
  }

  #[tokio::test]
  async fn policy_strict_requires_integrity() {
    let err = verify_integrity(
      IntegrityPolicy::Strict,
      &IntegrityChecker::Default,
      None,
      DATA,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, crate::Error::IntegrityVerifyFailed));
  }
}
