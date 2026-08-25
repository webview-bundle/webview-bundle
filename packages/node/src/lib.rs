//! Node-API bindings for the [`wvb`] core crate.
//!
//! This crate defines the JavaScript-facing API exported by `@wvb/node`. The generated
//! TypeScript declarations preserve the rustdoc comments on `#[napi]` items.

// N-API exports use standard JSDoc optional-parameter syntax such as `[options]`. Rustdoc parses
// that syntax as an intra-doc link, while the TypeScript declaration generator preserves it as
// intended JSDoc.
#![allow(rustdoc::broken_intra_doc_links, rustdoc::invalid_html_tags)]

/// Bundle encoding, decoding, and inspection APIs.
pub mod bundle;
/// Cancellation primitives for long-running native operations.
pub mod cancellation;
/// Constants shared with the core bundle format and update protocol.
pub mod consts;
mod error;
/// HTTP request and response types exposed to JavaScript callers.
pub mod http;
/// Integrity creation and verification APIs.
pub mod integrity;
/// JavaScript interoperation helpers used by the bindings.
pub mod js;
pub(crate) mod mime;
/// Custom-protocol handlers for bundle and proxy requests.
pub mod protocol;
/// Remote update discovery and bundle downloading APIs.
pub mod remote;
/// Signature verification APIs.
pub mod signature;
/// Local builtin and remote bundle source APIs.
pub mod source;
/// Download, install, and rollback APIs for remote updates.
pub mod updater;
/// Bundle-format version types.
pub mod version;

/// JavaScript-facing error type and helper result wrapper.
pub use error::{Error, Outcome};
/// Result type used internally by the Node-API bindings.
pub type Result<T> = std::result::Result<T, Error>;
