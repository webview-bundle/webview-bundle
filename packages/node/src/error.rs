use wvb::http;

#[derive(thiserror::Error, Debug)]
pub enum Error {
  #[error(transparent)]
  Core(#[from] wvb::Error),
  #[error(transparent)]
  InvalidHeaderName(#[from] http::header::InvalidHeaderName),
  #[error(transparent)]
  InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),
  #[error("{0}")]
  InvalidSignatureOptions(String),
  #[error(transparent)]
  Napi(#[from] napi::Error),
}

impl Error {
  pub(crate) fn invalid_signature_options(message: impl Into<String>) -> Self {
    Self::InvalidSignatureOptions(message.into())
  }
}

fn tagged(code: &str, error: &impl std::fmt::Display) -> String {
  format!("[code={code}] {error}")
}

impl From<Error> for napi::Error {
  fn from(value: Error) -> Self {
    match value {
      Error::Core(e) => napi::Error::new(
        napi::Status::GenericFailure,
        tagged(&format!("core.{}", e.code()), &e.to_string()),
      ),
      Error::InvalidHeaderName(e) => {
        napi::Error::new(napi::Status::InvalidArg, tagged("invalid_header_name", &e))
      }
      Error::InvalidHeaderValue(e) => {
        napi::Error::new(napi::Status::InvalidArg, tagged("invalid_header_value", &e))
      }
      Error::InvalidSignatureOptions(message) => napi::Error::new(
        napi::Status::InvalidArg,
        tagged("invalid_signature_options", &message),
      ),
      Error::Napi(e) => e,
    }
  }
}

impl From<Error> for napi::JsError {
  fn from(value: Error) -> Self {
    napi::JsError::from(napi::Error::from(value))
  }
}
