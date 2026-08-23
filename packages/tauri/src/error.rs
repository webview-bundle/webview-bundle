use serde::{
  Serialize,
  ser::{SerializeMap, Serializer},
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("webview bundle error: {0}")]
  Core(#[from] wvb::Error),
  #[error("fail to resolve path: {0}")]
  FailToResolvePath(String),
  #[error("tauri error: {0}")]
  Tauri(#[from] tauri::Error),
  #[error("remote is not initialized")]
  RemoteIsNotInitialized,
  #[error("updater is not initialized")]
  UpdaterIsNotInitialized,
}

impl From<std::io::Error> for Error {
  fn from(value: std::io::Error) -> Self {
    Self::Core(value.into())
  }
}

impl Error {
  /// Expose "code" field so that can be used in bridge.
  fn code(&self) -> String {
    match self {
      Error::Core(e) => e.code().to_string(),
      Error::FailToResolvePath(_) => "fail_to_resolve_directory".to_string(),
      Error::Tauri(_) => "tauri".to_string(),
      Error::RemoteIsNotInitialized => "remote_not_initialized".to_owned(),
      Error::UpdaterIsNotInitialized => "updater_not_initialized".to_owned(),
    }
  }
}

impl Serialize for Error {
  fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut map = serializer.serialize_map(Some(2))?;
    map.serialize_entry("message", &self.to_string())?;
    map.serialize_entry("code", &self.code())?;
    map.end()
  }
}
