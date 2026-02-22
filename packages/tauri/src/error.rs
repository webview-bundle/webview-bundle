use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("webview bundle error: {0}")]
  WebviewBundle(#[from] wvb::Error),
  #[error("fail to resolve directory: {0}")]
  FailToResolveDirectory(String),
  #[error("tauri error: {0}")]
  Tauri(#[from] tauri::Error),
  #[error("duplicated protocol scheme: {scheme}")]
  ProtocolSchemeDuplicated { scheme: String },
}

impl Serialize for Error {
  fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(self.to_string().as_ref())
  }
}
