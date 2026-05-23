use crate::bundle::Bundle;
use std::sync::Arc;
use wvb::remote;

#[derive(uniffi::Record, Clone, Debug)]
pub struct ListRemoteBundleInfo {
  pub name: String,
  pub version: String,
}

impl From<remote::ListRemoteBundleInfo> for ListRemoteBundleInfo {
  fn from(value: remote::ListRemoteBundleInfo) -> Self {
    Self {
      name: value.name,
      version: value.version,
    }
  }
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct RemoteBundleInfo {
  pub name: String,
  pub version: String,
  pub etag: Option<String>,
  pub integrity: Option<String>,
  pub signature: Option<String>,
  pub last_modified: Option<String>,
}

impl From<remote::RemoteBundleInfo> for RemoteBundleInfo {
  fn from(value: remote::RemoteBundleInfo) -> Self {
    Self {
      name: value.name,
      version: value.version,
      etag: value.etag,
      integrity: value.integrity,
      signature: value.signature,
      last_modified: value.last_modified,
    }
  }
}

impl From<RemoteBundleInfo> for remote::RemoteBundleInfo {
  fn from(value: RemoteBundleInfo) -> Self {
    Self {
      name: value.name,
      version: value.version,
      etag: value.etag,
      integrity: value.integrity,
      signature: value.signature,
      last_modified: value.last_modified,
    }
  }
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct DownloadResult {
  pub info: RemoteBundleInfo,
  pub bundle: Arc<Bundle>,
  pub data: Vec<u8>,
}

#[derive(uniffi::Object)]
pub struct Remote {
  pub(crate) inner: Arc<remote::Remote>,
}

#[uniffi::export]
impl Remote {
  #[uniffi::constructor]
  pub fn new(endpoint: String) -> Result<Arc<Remote>, crate::Error> {
    let inner = remote::Remote::builder().endpoint(endpoint).build()?;
    Ok(Arc::new(Remote {
      inner: Arc::new(inner),
    }))
  }
}

#[uniffi::export(async_runtime = "tokio")]
impl Remote {
  pub async fn list_bundles(
    &self,
    channel: Option<String>,
  ) -> Result<Vec<ListRemoteBundleInfo>, crate::Error> {
    let bundles = self
      .inner
      .list_bundles(channel.as_ref())
      .await?
      .into_iter()
      .map(ListRemoteBundleInfo::from)
      .collect();
    Ok(bundles)
  }

  pub async fn get_info(
    &self,
    bundle_name: String,
    channel: Option<String>,
  ) -> Result<RemoteBundleInfo, crate::Error> {
    let info = self
      .inner
      .get_current_info(&bundle_name, channel.as_ref())
      .await?;
    Ok(info.into())
  }

  pub async fn download(
    &self,
    bundle_name: String,
    channel: Option<String>,
  ) -> Result<DownloadResult, crate::Error> {
    let (info, inner, data) = self.inner.download(&bundle_name, channel.as_ref()).await?;
    Ok(DownloadResult {
      info: info.into(),
      bundle: Arc::new(Bundle {
        inner: Arc::new(inner),
      }),
      data: data.into(),
    })
  }

  pub async fn download_version(
    &self,
    bundle_name: String,
    version: String,
  ) -> Result<DownloadResult, crate::Error> {
    let (info, inner, data) = self.inner.download_version(&bundle_name, &version).await?;
    Ok(DownloadResult {
      info: info.into(),
      bundle: Arc::new(Bundle {
        inner: Arc::new(inner),
      }),
      data: data.into(),
    })
  }
}
