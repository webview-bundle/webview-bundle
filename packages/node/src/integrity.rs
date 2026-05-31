use napi_derive::napi;
use wvb::integrity;

/// Hash algorithm for bundle integrity verification.
///
/// Supports SHA-2 family hash algorithms for cryptographic verification
/// following the Subresource Integrity specification.
///
/// @example
/// ```typescript
/// // Integrity strings use these algorithms:
/// // "sha256-abc123..." - SHA-256
/// // "sha384-def456..." - SHA-384 (recommended)
/// // "sha512-ghi789..." - SHA-512
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
  None,
}

impl From<integrity::IntegrityPolicy> for IntegrityPolicy {
  fn from(value: integrity::IntegrityPolicy) -> Self {
    match value {
      integrity::IntegrityPolicy::Strict => Self::Strict,
      integrity::IntegrityPolicy::Optional => Self::Optional,
      integrity::IntegrityPolicy::None => Self::None,
    }
  }
}

impl From<IntegrityPolicy> for integrity::IntegrityPolicy {
  fn from(value: IntegrityPolicy) -> Self {
    match value {
      IntegrityPolicy::Strict => Self::Strict,
      IntegrityPolicy::Optional => Self::Optional,
      IntegrityPolicy::None => Self::None,
    }
  }
}
