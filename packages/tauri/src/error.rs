use serde::{
  Serialize,
  ser::{SerializeMap, Serializer},
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("webview bundle error: {0}")]
  Core(#[from] wvb::Error),
  #[error("fail to resolve directory: {0}")]
  FailToResolveDirectory(String),
  #[error("tauri error: {0}")]
  Tauri(#[from] tauri::Error),
  #[error("duplicated protocol scheme: {scheme}")]
  ProtocolSchemeDuplicated { scheme: String },
  #[error("remote is not initialized")]
  RemoteIsNotInitialized,
  #[error("updater is not initialized")]
  UpdaterIsNotInitialized,
}

impl Error {
  /// Expose "code" field so that can be used in bridge.
  fn code(&self) -> Option<String> {
    match self {
      Error::RemoteIsNotInitialized => Some("remote_not_initialized".to_owned()),
      Error::UpdaterIsNotInitialized => Some("updater_not_initialized".to_owned()),
      _ => None,
    }
  }
}

impl Serialize for Error {
  fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let code = self.code();
    let mut map = serializer.serialize_map(Some(if code.is_some() { 2 } else { 1 }))?;
    map.serialize_entry("message", &self.to_string())?;
    if let Some(code) = &code {
      map.serialize_entry("code", code)?;
    }
    map.end()
  }
}
