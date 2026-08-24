use crate::integrity::{Integrity, IntegrityPolicy};
use std::str::FromStr;

/// Verifies `data` against the `integrity` advertised for it, as `policy` dictates.
pub(crate) fn verify_integrity(
  policy: &IntegrityPolicy,
  integrity: Option<&str>,
  data: &[u8],
) -> crate::Result<()> {
  if policy == &IntegrityPolicy::Off {
    return Ok(());
  }
  match integrity {
    Some(integrity) => {
      let integrity = Integrity::from_str(integrity)?;
      if !integrity.validate(data) {
        return Err(crate::Error::IntegrityVerifyFailed);
      }
      Ok(())
    }
    None if policy == &IntegrityPolicy::Strict => Err(crate::Error::IntegrityVerifyFailed),
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

  #[test]
  fn policy_off_skips_the_check() {
    // Even a wrong integrity string is not looked at.
    let wrong = integrity_of(b"other bytes");
    verify_integrity(&IntegrityPolicy::Off, Some(&wrong), DATA).unwrap();
  }

  #[test]
  fn policy_optional_checks_when_present() {
    verify_integrity(&IntegrityPolicy::Optional, Some(&integrity_of(DATA)), DATA).unwrap();
    verify_integrity(&IntegrityPolicy::Optional, None, DATA).unwrap();
    let err = verify_integrity(
      &IntegrityPolicy::Optional,
      Some(&integrity_of(b"other")),
      DATA,
    )
    .unwrap_err();
    assert!(matches!(err, crate::Error::IntegrityVerifyFailed));
  }

  #[test]
  fn policy_strict_requires_integrity() {
    let err = verify_integrity(&IntegrityPolicy::Strict, None, DATA).unwrap_err();
    assert!(matches!(err, crate::Error::IntegrityVerifyFailed));
  }
}
