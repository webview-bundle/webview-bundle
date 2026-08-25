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
//! # {
//! use std::str::FromStr;
//! use wvb::integrity::{Integrity, IntegrityAlgorithm};
//!
//! let data = b"<html></html>";
//!
//! // Compute an integrity string over some bytes.
//! let integrity = Integrity::compute(IntegrityAlgorithm::Sha256, data).serialize();
//! println!("Integrity: {integrity}");
//!
//! // Verify bytes against it.
//! assert!(Integrity::from_str(&integrity).unwrap().validate(data));
//! # }
//! ```
//!
//! ## Integrity Policy
//!
//! [`crate::integrity::IntegrityPolicy`] controls how a bundle's integrity metadata is treated when
//! the check runs — required ([`crate::integrity::IntegrityPolicy::Strict`]), checked when present
//! ([`crate::integrity::IntegrityPolicy::Optional`]), or disabled
//! ([`crate::integrity::IntegrityPolicy::Off`]). It is applied
//! through [`crate::source::SourceOptions::integrity`] (on load) and
//! [`crate::updater::UpdaterOptions::integrity`] (on install).
//!
//! ```no_run
//! # #[cfg(all(feature = "integrity", feature = "source"))]
//! # {
//! use wvb::integrity::IntegrityPolicy;
//! use wvb::source::{SourceIntegrityOptions, SourceOptions};
//!
//! // Require integrity metadata on every bundle this source verifies on load.
//! let options = SourceOptions::default()
//!     .integrity(SourceIntegrityOptions::default().policy(IntegrityPolicy::Strict));
//! # let _ = options;
//! # }
//! ```

mod integrity;
mod policy;
mod verify;

pub use integrity::*;
pub use policy::*;
pub(crate) use verify::*;
