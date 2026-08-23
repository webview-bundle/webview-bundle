use crate::remote::RemoteUpdateResponse;
use crate::util;
use std::path::{Path, PathBuf};
use tokio::sync::{Mutex, OnceCell};

#[derive(Debug)]
pub(crate) struct UpdateFile {
  filepath: PathBuf,
  data: OnceCell<Mutex<Option<RemoteUpdateResponse>>>,
}

impl UpdateFile {
  pub fn new(filepath: &Path) -> Self {
    Self {
      filepath: filepath.to_path_buf(),
      data: Default::default(),
    }
  }

  pub async fn read(&self) -> crate::Result<Option<RemoteUpdateResponse>> {
    let data = self.load().await?.lock().await;
    Ok(data.clone())
  }

  pub async fn write(&self, response: &RemoteUpdateResponse) -> crate::Result<()> {
    let mut data = self.load().await?.lock().await;
    let raw = serde_json::to_vec(response)?;
    util::fs::atomic_write_file(&self.filepath, &raw).await?;
    *data = Some(response.clone());
    Ok(())
  }

  async fn load(&self) -> crate::Result<&Mutex<Option<RemoteUpdateResponse>>> {
    let data = self
      .data
      .get_or_try_init(|| async {
        let data = util::fs::read_file_with_retry(&self.filepath)
          .await
          .ok()
          .and_then(|x| serde_json::from_slice::<RemoteUpdateResponse>(&x).ok());
        Ok::<Mutex<Option<RemoteUpdateResponse>>, crate::Error>(Mutex::new(data))
      })
      .await?;
    Ok(data)
  }
}

#[cfg(all(test, feature = "testing"))]
mod tests {
  use super::*;
  use crate::remote::Update;
  use crate::testing::TempDir;
  use std::collections::HashMap;

  fn response(id: &str) -> RemoteUpdateResponse {
    RemoteUpdateResponse {
      update: Update {
        id: id.to_owned(),
        created_at: "2026-08-08T00:00:00Z".to_owned(),
        runtime_version: 1,
        bundles: vec![],
        metadata: HashMap::new(),
      },
      etag: Some("\"etag-1\"".to_owned()),
      signature: None,
    }
  }

  #[tokio::test]
  async fn reads_none_when_file_not_exists() {
    let temp = TempDir::new();
    let file = UpdateFile::new(&temp.dir().join("update.json"));

    assert_eq!(file.read().await.unwrap(), None);
  }

  #[tokio::test]
  async fn writes_and_reads() {
    let temp = TempDir::new();
    let file = UpdateFile::new(&temp.dir().join("update.json"));

    file.write(&response("u1")).await.unwrap();

    assert_eq!(file.read().await.unwrap(), Some(response("u1")));
  }

  #[tokio::test]
  async fn writes_to_file() {
    let temp = TempDir::new();
    let filepath = temp.dir().join("update.json");

    UpdateFile::new(&filepath)
      .write(&response("u1"))
      .await
      .unwrap();

    let read = UpdateFile::new(&filepath).read().await.unwrap();
    assert_eq!(read, Some(response("u1")));
  }

  #[tokio::test]
  async fn overwrites_written_data() {
    let temp = TempDir::new();
    let filepath = temp.dir().join("update.json");
    let file = UpdateFile::new(&filepath);
    file.write(&response("u1")).await.unwrap();

    file.write(&response("u2")).await.unwrap();

    assert_eq!(file.read().await.unwrap(), Some(response("u2")));
    let read = UpdateFile::new(&filepath).read().await.unwrap();
    assert_eq!(read, Some(response("u2")));
  }
}
