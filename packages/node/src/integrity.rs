use crate::js::JsCallback;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::str::FromStr;
use wvb::integrity;

pub(crate) type IntegrityChecker = JsCallback<FnArgs<(Buffer, String)>, Promise<bool>>;

/// Hash algorithm for bundle integrity verification.
///
/// Supports SHA-2 family hash algorithms for cryptographic verification.
///
/// @example
/// ```typescript
/// // Integrity strings are `<algorithm>:<base64>`:
/// // "sha256:abc123..." - SHA-256
/// // "sha384:def456..." - SHA-384 (recommended)
/// // "sha512:ghi789..." - SHA-512
/// ```
#[napi(string_enum = "camelCase")]
pub enum IntegrityAlgorithm {
  /// SHA-256 (256-bit hash)
  Sha256,
  /// SHA-384 (384-bit hash, recommended)
  Sha384,
  /// SHA-512 (512-bit hash)
  Sha512,
}

impl From<integrity::IntegrityAlgorithm> for IntegrityAlgorithm {
  fn from(value: integrity::IntegrityAlgorithm) -> Self {
    match value {
      integrity::IntegrityAlgorithm::Sha256 => Self::Sha256,
      integrity::IntegrityAlgorithm::Sha384 => Self::Sha384,
      integrity::IntegrityAlgorithm::Sha512 => Self::Sha512,
    }
  }
}

impl From<IntegrityAlgorithm> for integrity::IntegrityAlgorithm {
  fn from(value: IntegrityAlgorithm) -> Self {
    match value {
      IntegrityAlgorithm::Sha256 => integrity::IntegrityAlgorithm::Sha256,
      IntegrityAlgorithm::Sha384 => integrity::IntegrityAlgorithm::Sha384,
      IntegrityAlgorithm::Sha512 => integrity::IntegrityAlgorithm::Sha512,
    }
  }
}

/// Computes the integrity of `data` with `algorithm`.
///
/// This is the write side of integrity: use it when publishing a bundle to produce the
/// string a source or updater later verifies against.
///
/// @param {IntegrityAlgorithm} algorithm - Hash algorithm to digest with
/// @param {Buffer} data - Bytes to digest
/// @returns {Integrity} The computed integrity
///
/// @example
/// ```typescript
/// import { computeIntegrity } from '@wvb/node';
///
/// const integrity = computeIntegrity('sha256', data).serialize();
/// // "sha256:n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg="
/// ```
#[napi]
pub fn compute_integrity(algorithm: IntegrityAlgorithm, data: Buffer) -> Integrity {
  Integrity {
    inner: integrity::Integrity::compute(algorithm.into(), &data),
  }
}

/// Parses a serialized integrity string (e.g. `"sha256:n4bQ..."`).
///
/// @param {string} integrity - Serialized `<algorithm>:<base64>` string
/// @returns {Integrity} The parsed integrity
///
/// @example
/// ```typescript
/// import { parseIntegrity } from '@wvb/node';
///
/// const isValid = parseIntegrity(advertised).validate(data);
/// ```
#[napi]
pub fn parse_integrity(integrity: String) -> crate::Outcome<Integrity> {
  crate::Outcome::from_fn(|| {
    Ok(Integrity {
      inner: integrity::Integrity::from_str(&integrity)?,
    })
  })
}

/// A digest over some bytes, serialized as `<algorithm>:<base64>` (e.g. `"sha256:n4bQ..."`).
///
/// Created by [`computeIntegrity`] or [`parseIntegrity`].
#[napi]
pub struct Integrity {
  inner: integrity::Integrity,
}

#[napi]
impl Integrity {
  /// The raw digest bytes.
  ///
  /// @returns {Buffer} The digest
  #[napi]
  pub fn value(&self) -> Buffer {
    self.inner.value().to_vec().into()
  }

  /// Whether `data` digests to this integrity.
  ///
  /// @param {Buffer} data - Bytes to check
  /// @returns {boolean} `true` when the bytes match
  #[napi]
  pub fn validate(&self, data: Buffer) -> bool {
    self.inner.validate(&data)
  }

  /// Serializes to `<algorithm>:<base64>`.
  ///
  /// @returns {string} The serialized integrity string
  #[napi]
  pub fn serialize(&self) -> String {
    self.inner.serialize()
  }
}

/// Policy for enforcing integrity verification during bundle operations.
///
/// Controls when integrity hashes are required and how missing hashes are handled.
///
/// @example
/// ```typescript
/// import { Updater } from '@wvb/node';
///
/// // Require integrity for all bundles
/// const updater = new Updater(source, remote, {
///   integrityPolicy: 'strict',
/// });
///
/// // Optional integrity (warn if missing)
/// const updater2 = new Updater(source, remote, {
///   integrityPolicy: 'optional',
/// });
/// ```
#[napi(string_enum = "camelCase")]
pub enum IntegrityPolicy {
  /// Require integrity verification for all bundles. Operations fail if integrity is missing or invalid.
  Strict,
  /// Verify integrity if provided, but allow operations without it.
  Optional,
  /// Skip integrity verification entirely.
  Off,
}

impl From<integrity::IntegrityPolicy> for IntegrityPolicy {
  fn from(value: integrity::IntegrityPolicy) -> Self {
    match value {
      integrity::IntegrityPolicy::Strict => Self::Strict,
      integrity::IntegrityPolicy::Optional => Self::Optional,
      integrity::IntegrityPolicy::Off => Self::Off,
    }
  }
}

impl From<IntegrityPolicy> for integrity::IntegrityPolicy {
  fn from(value: IntegrityPolicy) -> Self {
    match value {
      IntegrityPolicy::Strict => Self::Strict,
      IntegrityPolicy::Optional => Self::Optional,
      IntegrityPolicy::Off => Self::Off,
    }
  }
}
