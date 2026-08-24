//! FFI bindings for the WebViewBundle library, generated via [UniFFI](https://mozilla.github.io/uniffi-rs/).
//!
//! Each module exposes a thin wrapper over the corresponding `wvb` core type,
//! translating between Rust-native types and the flattened record/enum/object
//! types that UniFFI can project into Kotlin, Swift, and other target languages.

/// Bundle encoding, decoding, and inspection APIs.
pub mod bundle;
/// Cancellation primitives for asynchronous operations.
pub mod cancellation;
/// Constants shared with the core bundle format and update protocol.
pub mod consts;
/// Error values returned by the FFI API.
pub mod error;
/// HTTP request and response types used by protocol APIs.
pub mod http;
/// Bundle integrity creation and verification APIs.
pub mod integrity;
/// MIME-type helpers.
pub mod mime;
/// Bundle and proxy protocol APIs.
pub mod protocol;
/// Remote update discovery and download APIs.
pub mod remote;
/// Signature verification APIs.
pub mod signature;
/// Builtin and downloaded bundle source APIs.
pub mod source;
/// Download, install, and rollback APIs for remote updates.
pub mod updater;
/// Bundle-format version types.
pub mod version;

/// FFI error type.
pub use error::Error;
/// Convenience alias used throughout the FFI layer so every fallible function
/// returns the same [`Error`] type without repeating it.
pub type Result<T> = std::result::Result<T, Error>;

uniffi::setup_scaffolding!();
