/// Top-level error type exposed across the FFI boundary.
///
/// Errors are flattened to string messages (`flat_error`) because structured
/// error types cannot be projected into all UniFFI target languages.
#[derive(thiserror::Error, uniffi::Error, Debug)]
#[uniffi(flat_error)]
pub enum Error {
  /// Propagated from the `wvb` core library.
  #[error("{0}")]
  Core(String),
  /// Invalid HTTP header name or value.
  #[error("{0}")]
  Http(String),
  /// Signature key parsing or verification failure.
  #[error("{0}")]
  Signature(String),
}

impl From<wvb::Error> for Error {
  fn from(e: wvb::Error) -> Self {
    Error::Core(e.to_string())
  }
}

impl From<http::header::InvalidHeaderName> for Error {
  fn from(e: http::header::InvalidHeaderName) -> Self {
    Error::Http(e.to_string())
  }
}

impl From<http::header::InvalidHeaderValue> for Error {
  fn from(e: http::header::InvalidHeaderValue) -> Self {
    Error::Http(e.to_string())
  }
}
