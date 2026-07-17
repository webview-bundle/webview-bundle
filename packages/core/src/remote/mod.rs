//! HTTP client for downloading bundles from a remote server.
//!
//! The remote module implements the client side of the bundle HTTP protocol,
//! allowing applications to:
//!
//! - List available bundles on a server
//! - Fetch bundle metadata (version, integrity, signature)
//! - Download specific bundle versions
//! - Verify bundle integrity before installation
//!
//! ## HTTP API Endpoints
//!
//! - `GET /bundles` - List all available bundles
//! - `HEAD /bundles/{name}` - Get bundle metadata without downloading
//! - `GET /bundles/{name}` - Download the current version of a bundle
//! - `GET /bundles/{name}/{version}` - Download a specific version
//!
//! ## Example
//!
//! ```no_run
//! # #[cfg(all(feature = "remote", feature = "source"))]
//! # async {
//! use wvb::remote::Remote;
//! use wvb::source::{BundleManifestMetadata, BundleSource};
//!
//! let remote = Remote::builder()
//!     .endpoint("https://updates.example.com")
//!     .build()
//!     .unwrap();
//! let source = BundleSource::builder()
//!     .remote_dir("./remote")
//!     .build();
//!
//! // List available bundles
//! let bundles = remote.list_bundles(None).await.unwrap();
//!
//! // Get bundle info without downloading it
//! let info = remote.get_current_info("app", None).await.unwrap();
//! println!("Latest version: {}", info.version);
//!
//! // Download and install
//! let (info, _bundle, data) = remote.download("app", None).await.unwrap();
//! source
//!     .write_remote_bundle_data(
//!         "app",
//!         &info.version,
//!         &data,
//!         BundleManifestMetadata {
//!             etag: info.etag,
//!             integrity: info.integrity,
//!             signature: info.signature,
//!             last_modified: info.last_modified,
//!         },
//!     )
//!     .await
//!     .unwrap();
//! source.update_remote_version("app", &info.version).await.unwrap();
//! # };
//! ```
//!
//! ## Headers
//!
//! Bundle metadata is communicated via HTTP headers:
//!
//! - `Webview-Bundle-Name`: Bundle identifier
//! - `Webview-Bundle-Version`: Version string
//! - `Webview-Bundle-Integrity`: Optional integrity hash for verification
//! - `Webview-Bundle-Signature`: Optional digital signature

mod dto;
mod http;
mod options;
mod remote;

pub use dto::*;
pub use http::*;
pub use options::*;
pub use remote::*;
