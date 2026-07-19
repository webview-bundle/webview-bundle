#![allow(dead_code)]

use crate::result::{WvbResult, core_err, ok_result};
use crate::{cstr, owned_bytes};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::ffi::c_char;
use std::str::FromStr;
use wvb::integrity::{
  Integrity, IntegrityAlgorithm as CoreIntegrityAlgorithm, IntegrityPolicy as CoreIntegrityPolicy,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum IntegrityAlgorithm {
  Sha256,
  Sha384,
  Sha512,
}

/// How a bundle's integrity metadata is treated when the integrity check runs.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum IntegrityPolicy {
  Strict,
  Optional,
  Off,
}

/// `integrity.policy`/`integrityPolicy` string mapping shared by the source and the updater.
/// Returns `None` for an unknown value, so the caller can fail closed rather than pick a default.
pub(crate) fn parse_integrity_policy(policy: &str) -> Option<CoreIntegrityPolicy> {
  match policy {
    "strict" => Some(CoreIntegrityPolicy::Strict),
    "optional" => Some(CoreIntegrityPolicy::Optional),
    "off" => Some(CoreIntegrityPolicy::Off),
    _ => None,
  }
}

/// Compute the integrity of `data` under `algorithm` (`"sha256"`/`"sha384"`/`"sha512"`).
/// The result's json is the serialized `<algorithm>:<base64>` string and its body is the
/// raw digest.
///
/// # Safety
/// `algorithm` must be a valid C string; `data` must be null or point to `data_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_compute_integrity(
  algorithm: *const c_char,
  data: *const u8,
  data_len: usize,
) -> *mut WvbResult {
  let raw = unsafe { cstr(algorithm) };
  let algorithm = match CoreIntegrityAlgorithm::from_str(&raw) {
    Ok(algorithm) => algorithm,
    Err(e) => return core_err(e),
  };
  let integrity = Integrity::compute(algorithm, &unsafe { owned_bytes(data, data_len) });
  ok_result(
    serde_json::Value::String(integrity.serialize()),
    integrity.value().to_vec(),
  )
}

/// Parse a serialized `<algorithm>:<base64>` integrity string. The result's json is the
/// re-serialized string and its body is the raw digest.
///
/// # Safety
/// `integrity` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_parse_integrity(integrity: *const c_char) -> *mut WvbResult {
  let raw = unsafe { cstr(integrity) };
  match Integrity::from_str(&raw) {
    Ok(integrity) => ok_result(
      serde_json::Value::String(integrity.serialize()),
      integrity.value().to_vec(),
    ),
    Err(e) => core_err(e),
  }
}

/// Whether `data` digests to `integrity`. The result's json is the boolean.
///
/// # Safety
/// `integrity` must be a valid C string; `data` must be null or point to `data_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_integrity_validate(
  integrity: *const c_char,
  data: *const u8,
  data_len: usize,
) -> *mut WvbResult {
  let raw = unsafe { cstr(integrity) };
  match Integrity::from_str(&raw) {
    Ok(integrity) => ok_result(
      serde_json::Value::Bool(integrity.validate(&unsafe { owned_bytes(data, data_len) })),
      Vec::new(),
    ),
    Err(e) => core_err(e),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn integrity_policy_fails_closed_on_an_unknown_value() {
    assert!(matches!(
      parse_integrity_policy("strict"),
      Some(CoreIntegrityPolicy::Strict)
    ));
    assert!(matches!(
      parse_integrity_policy("optional"),
      Some(CoreIntegrityPolicy::Optional)
    ));
    assert!(matches!(
      parse_integrity_policy("off"),
      Some(CoreIntegrityPolicy::Off)
    ));
    // The old spelling of 'off' must not silently map to a different policy.
    assert!(parse_integrity_policy("none").is_none());
  }
}
