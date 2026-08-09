//! Bundle integrity verification using cryptographic hashes.
//!
//! ## Integrity Format
//!
//! Integrity hashes are formatted as `<algorithm>:<base64-hash>`.
//!
//! ```text
//! sha256:base64hash...
//! ```
//!
//! ## Example
//!
//! ```no_run
//! # #[cfg(feature = "integrity")]
//! # async {
//! use wvb::integrity::{Integrity, IntegrityAlgorithm, IntegrityCheck};
//!
//! let data = b"<html></html>";
//!
//! // Compute an integrity string over some bytes.
//! let integrity = Integrity::compute(IntegrityAlgorithm::Sha256, data).serialize();
//! println!("Integrity: {integrity}");
//!
//! // Verify bytes against it.
//! IntegrityCheck::Default.check(&integrity, data).await.unwrap();
//! # };
//! ```
//!
//! ## Integrity Policy
//!
//! [`IntegrityPolicy`] controls how a bundle's integrity metadata is treated when the
//! check runs — required ([`IntegrityPolicy::Strict`]), checked when present
//! ([`IntegrityPolicy::Optional`]), or disabled ([`IntegrityPolicy::Off`]). It is applied
//! through [`crate::source::BundleSourceOptions::integrity`] (on load) and
//! [`crate::updater::UpdaterOptions::integrity_policy`] (on download/install).
//!
//! ```no_run
//! # #[cfg(all(feature = "integrity", feature = "source"))]
//! # {
//! use wvb::integrity::IntegrityPolicy;
//! use wvb::source::{BundleSourceIntegrityOptions, BundleSourceOptions};
//!
//! // Require integrity metadata on every bundle this source verifies on load.
//! let options = BundleSourceOptions::default()
//!     .integrity(BundleSourceIntegrityOptions::default().policy(IntegrityPolicy::Strict));
//! # let _ = options;
//! # }
//! ```

mod integrity;
mod policy;
mod verify;

pub use integrity::*;
pub use policy::*;
pub use verify::*;
