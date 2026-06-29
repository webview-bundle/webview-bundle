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
//! use wvb::integrity::{Integrity, IntegrityChecker};
//! use wvb::{Bundle, BundleBuilder};
//!
//! // Create a bundle
//! let bundle = Bundle::builder()
//!     .add_file("/index.html", b"<html></html>", None)
//!     .build();
//!
//! // Generate integrity hash
//! let integrity = Integrity::from_bundle(&bundle).unwrap();
//! let hash_string = integrity.to_string();
//! println!("Integrity: {}", hash_string);
//!
//! // Verify integrity
//! let checker = IntegrityChecker::new(hash_string);
//! assert!(checker.verify(&bundle).is_ok());
//! # };
//! ```
//!
//! ## Integrity Policy
//!
//! Control when integrity verification is required:
//!
//! ```no_run
//! # #[cfg(all(feature = "integrity", feature = "remote"))]
//! # async {
//! use wvb::integrity::IntegrityPolicy;
//! use wvb::remote::{Remote, RemoteConfig};
//!
//! // Require integrity for all downloads
//! let config = RemoteConfig::default()
//!     .integrity_policy(IntegrityPolicy::RequireForAll);
//!
//! let remote = Remote::new_with_config("https://updates.example.com", config);
//! # };
//! ```

mod checker;
mod integrity;
mod policy;

pub use checker::*;
pub use integrity::*;
pub use policy::*;
