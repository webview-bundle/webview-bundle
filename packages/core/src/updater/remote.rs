/// Options for remote operations.
#[derive(Default, Clone)]
#[non_exhaustive]
pub struct RemoteOptions {}

/// Remote client for using with remote bundles.
#[derive(Clone)]
pub struct Remote {
  client: reqwest::Client,
}

impl Remote {
  /// Get updates from the remote server.
  pub async fn get_update(&self) -> crate::Result<()> {
    let mut headers = reqwest::header::HeaderMap::new();
    let mut req = self.client.get(self.endpoint("/update")?);
    let res = req.send().await?;
    todo!()
  }

  pub async fn download(&self) -> crate::Result<()> {
    todo!()
  }

  fn endpoint(&self, path: impl Into<String>) -> crate::Result<String> {
    todo!()
  }
}
