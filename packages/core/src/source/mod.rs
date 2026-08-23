//! Bundle source management for organizing multiple bundle versions.
//!
//! A **Source** is a local directory structure that stores multiple webview bundles with version
//! management through a `manifest.json` file.
//!
//! ## Directory Structure
//!
//! ```text
//! {source_dir}/
//! ├── {bundle_name}/
//! │   ├── {bundle_name}_{version}.wvb
//! │   └── {bundle_name}_{version}.wvb
//! ├── app/
//! │   ├── app_1.0.0.wvb
//! │   └── app_1.1.0.wvb
//! └── manifest.json
//! ```
//!
//! ## Builtin vs Remote Sources
//!
//! Applications typically use two source directories:
//!
//! - **`builtin`**: Bundles shipped with the application. Read-only, used as fallback.
//! - **`remote`**: Downloaded bundles. Takes priority when a bundle exists in both sources.
//!
//! ## Example
//!
//! ```no_run
//! # #[cfg(feature = "source")]
//! # async {
//! use wvb::source::Source;
//!
//! let source = Source::builder()
//!     .builtin_dir("./builtin")
//!     .remote_dir("./remote")
//!     .build();
//!
//! // Remote version takes priority over builtin
//! let version = source.get_version("app").await.unwrap();
//! let bundle = source.fetch_bundle("app").await.unwrap();
//!
//! // List all available bundles
//! let bundles = source.list_bundles().await.unwrap();
//! # };
//! ```
//!
//! ## Manifest Format
//!
//! The `manifest.json` file tracks bundle versions and metadata:
//!
//! ```json
//! {
//!   "manifestVersion": 1,
//!   "entries": {
//!     "app": {
//!       "versions": {
//!         "1.0.0": {
//!           "integrity": "sha256:...",
//!           "metadata": { "channel": "stable" }
//!         }
//!       },
//!       "currentVersion": "1.0.0",
//!       "previousVersion": "0.9.0",
//!       "stagedVersion": "1.1.0"
//!     }
//!   }
//! }
//! ```

mod manifest;
mod manifest_data;
mod source;
mod types;
mod version;

pub use manifest::*;
pub use manifest_data::*;
pub use source::*;
pub use types::*;
pub use version::*;
