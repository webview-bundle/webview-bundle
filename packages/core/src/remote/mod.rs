//! HTTP client for get updates or downloading bundles from a remote server.
//!
//! ## Example
//!
//! ```no_run
//! use wvb::remote::Remote;
//!
//! let remote = Remote::builder()
//!   .base_url("https://my-update-server.com")
//!   .build()
//!   .unwrap();
//!
//! // Get update
//! let update = remote.get_update(None).await.unwrap();
//!
//! // Download bundle
//! remote.download(
//!   "https://my-bundle-cdn.com/bundles/<some_unique_key>",
//!   "/tmp/dir/to/download/bundle/my_bundle.wvb",
//! None,
//! ).await.unwrap();
//! ```

mod config;
mod consts;
mod http;
mod remote;
pub(crate) mod sfv;
mod streaming;
mod tmp;
mod types;

pub use config::*;
pub use consts::*;
pub use http::*;
pub use remote::*;
pub use types::*;
