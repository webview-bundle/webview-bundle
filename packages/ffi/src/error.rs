#[derive(thiserror::Error, uniffi::Error, Debug)]
#[uniffi(flat_error)]
pub enum Error {
  #[error("{0}")]
  Core(String),
  #[error("{0}")]
  Http(String),
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
