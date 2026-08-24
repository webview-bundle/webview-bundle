//! FFI bindings for the WebViewBundle library, generated via [UniFFI](https://mozilla.github.io/uniffi-rs/).
//!
//! Each module exposes a thin wrapper over the corresponding `wvb` core type,
//! translating between Rust-native types and the flattened record/enum/object
//! types that UniFFI can project into Kotlin, Swift, and other target languages.

pub mod bundle;
pub mod cancellation;
pub mod consts;
pub mod error;
pub mod http;
pub mod integrity;
pub mod mime;
pub mod protocol;
pub mod remote;
pub mod signature;
pub mod source;
pub mod updater;
pub mod version;

pub use error::Error;
/// Convenience alias used throughout the FFI layer so every fallible function
/// returns the same [`Error`] type without repeating it.
pub type Result<T> = std::result::Result<T, Error>;

uniffi::setup_scaffolding!();
