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
//! use wvb::source::BundleSource;
//!
//! let source = BundleSource::builder()
//!     .builtin_dir("./builtin")
//!     .remote_dir("./remote")
//!     .build();
//!
//! // Remote version takes priority over builtin
//! let version = source.load_version("app").await.unwrap();
//! let bundle = source.fetch("app").await.unwrap();
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
//!           "etag": "...",
//!           "integrity": "...",
//!           "signature": "..."
//!         }
//!       },
//!       "currentVersion": "1.0.0"
//!     }
//!   }
//! }
//! ```

mod kind;
mod manifest;
mod options;
mod source;
mod utils;
mod version;

pub use kind::*;
pub use manifest::*;
pub use options::*;
pub use source::*;
pub use version::*;
