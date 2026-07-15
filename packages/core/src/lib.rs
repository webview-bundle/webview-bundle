//! # Webview Bundle Core
//!
//! An offline-first web resources delivery system for webview-mounted frameworks and platforms
//! (e.g., Electron, Tauri, with Android and iOS planned).
//!
//! ## Overview
//!
//! Webview Bundle provides a compressed, verified bundle format (`.wvb`) for delivering web
//! resources to webview applications. It supports:
//!
//! - **Offline-first architecture**: Bundle resources locally for immediate availability
//! - **Delta updates**: Download only what changed between versions
//! - **Integrity verification**: Ensure bundle authenticity with checksums and signatures
//! - **Source management**: Organize bundles with builtin and remote sources
//! - **HTTP protocol support**: Serve bundles through custom protocol handlers
//!
//! ## Bundle Format
//!
//! The `.wvb` format consists of three main parts:
//!
//! | Header (17 bytes) | Index (variable) | Data (variable) |
//! |-------------------|------------------|-----------------|
//! | Magic number, version, index size, checksum | File paths and metadata | Compressed file contents |
//!
//! - **Header**: Magic number (🌐🎁), format version, index size, and checksum
//! - **Index**: HashMap of file paths to offset/length/headers, with checksum
//! - **Data**: LZ4-compressed file contents with xxHash-32 checksums
//!
//! ## Quick Start
//!
//! ```no_run
//! use wvb::{Bundle, BundleBuilder};
//!
//! # async {
//! // Create a new bundle
//! let mut builder = BundleBuilder::new();
//! builder.add_file("/index.html", b"<html>...</html>", None);
//! builder.add_file("/app.js", b"console.log('hello');", None);
//! let bundle = builder.build();
//!
//! // Write to file
//! # use wvb::{AsyncBundleWriter, AsyncWriter};
//! # use tokio::fs::File;
//! let mut file = File::create("app.wvb").await.unwrap();
//! AsyncBundleWriter::new(&mut file).write(&bundle).await.unwrap();
//!
//! // Read from file
//! # use wvb::{AsyncBundleReader, AsyncReader};
//! let mut file = File::open("app.wvb").await.unwrap();
//! let bundle: Bundle = AsyncBundleReader::new(&mut file).read().await.unwrap();
//!
//! // Access files
//! let html = bundle.get_data("/index.html").unwrap().unwrap();
//! # };
//! ```
//!
//! ## Features
//!
//! - `async`: Async I/O support with tokio
//! - `source`: Bundle source management (builtin/remote)
//! - `remote`: HTTP client for downloading bundles
//! - `updater`: Automatic bundle updates
//! - `protocol`: Custom protocol handlers for serving bundles
//! - `protocol-proxy`: Proxy protocol to local servers
//! - `integrity`: SHA-2 based integrity verification
//! - `signature`: Digital signature verification (ECDSA, Ed25519, RSA)
//! - `full`: Enable all features
//!
//! ## Bundle Source
//!
//! Organize multiple bundle versions with the `BundleSource` API:
//!
//! ```no_run
//! # #[cfg(feature = "source")]
//! # async {
//! use wvb::source::BundleSource;
//!
//! let source = BundleSource::builder()
//!     .builtin_dir("./builtin")  // Shipped with app
//!     .remote_dir("./remote")     // Downloaded updates
//!     .build();
//!
//! // Load current version (remote takes priority)
//! let bundle = source.fetch("app").await.unwrap();
//! # };
//! ```
//!
//! ## Remote Updates
//!
//! Download and verify bundles from a remote server:
//!
//! ```no_run
//! # #[cfg(all(feature = "remote", feature = "source"))]
//! # async {
//! use wvb::remote::Remote;
//! use wvb::source::BundleSource;
//!
//! let remote = Remote::new("https://updates.example.com");
//! let source = BundleSource::builder()
//!     .remote_dir("./remote")
//!     .build();
//!
//! // Download and install update
//! let bundle_info = remote.fetch_bundle("app").await.unwrap();
//! remote.download_and_write(&source, "app", &bundle_info).await.unwrap();
//! # };
//! ```

mod builder;
mod bundle;
mod checksum;
mod error;
mod header;
mod index;
mod reader;
mod version;
mod writer;

pub(crate) type Result<T> = std::result::Result<T, Error>;

pub use builder::*;
pub use bundle::*;
pub use consts::*;
pub use error::{Error, ErrorCode};
pub use header::*;
pub use index::*;
pub use reader::*;
pub use version::*;
pub use writer::*;

pub use http;

mod consts;
#[cfg(feature = "integrity")]
pub mod integrity;
#[cfg(feature = "protocol")]
pub mod protocol;
#[cfg(feature = "remote")]
pub mod remote;
#[cfg(feature = "signature")]
pub mod signature;
#[cfg(feature = "source")]
pub mod source;
#[cfg(feature = "testing")]
pub mod testing;
#[cfg(feature = "updater")]
pub mod updater;
