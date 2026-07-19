use crate::bundle::Bundle;
use crate::http::HttpHeaders;
use std::collections::HashMap;
use std::sync::Arc;
use wvb::http::HeaderMap;
use wvb::remote;

/// HTTP client options
#[derive(uniffi::Record, Clone, Debug, Default)]
pub struct HttpOptions {
  /// Headers sent with every request.
  #[uniffi(default = None)]
  pub default_headers: Option<HashMap<String, String>>,
  #[uniffi(default = None)]
  pub user_agent: Option<String>,
  /// Total request timeout in milliseconds (default: `120000`).
  ///
  /// Bounds an otherwise-unbounded download: without it a stalled transfer would hang
  /// forever and keep holding the updater's per-bundle lock.
  #[uniffi(default = None)]
  pub timeout: Option<u64>,
  /// Timeout in milliseconds for reading the response body.
  #[uniffi(default = None)]
  pub read_timeout: Option<u64>,
  /// Timeout in milliseconds for establishing the connection.
  #[uniffi(default = None)]
  pub connect_timeout: Option<u64>,
  #[uniffi(default = None)]
  pub pool_idle_timeout: Option<u64>,
  #[uniffi(default = None)]
  pub pool_max_idle_per_host: Option<u32>,
  #[uniffi(default = None)]
  pub referer: Option<bool>,
  #[uniffi(default = None)]
  pub tcp_nodelay: Option<bool>,
}

impl TryFrom<HttpOptions> for remote::HttpOptions {
  type Error = crate::Error;

  fn try_from(value: HttpOptions) -> Result<Self, Self::Error> {
    let mut options = remote::HttpOptions::new();
    if let Some(default_headers) = value.default_headers {
      options = options.default_headers(HeaderMap::try_from(HttpHeaders::from(default_headers))?);
    }
    if let Some(user_agent) = value.user_agent {
      options = options.user_agent(user_agent);
    }
    if let Some(timeout) = value.timeout {
      options = options.timeout(timeout);
    }
    if let Some(read_timeout) = value.read_timeout {
      options = options.read_timeout(read_timeout);
    }
    if let Some(connect_timeout) = value.connect_timeout {
      options = options.connect_timeout(connect_timeout);
    }
    if let Some(pool_idle_timeout) = value.pool_idle_timeout {
      options = options.pool_idle_timeout(pool_idle_timeout);
    }
    if let Some(pool_max_idle_per_host) = value.pool_max_idle_per_host {
      options = options.pool_max_idle_per_host(pool_max_idle_per_host as usize);
    }
    if let Some(referer) = value.referer {
      options = options.referer(referer);
    }
    if let Some(tcp_nodelay) = value.tcp_nodelay {
      options = options.tcp_nodelay(tcp_nodelay);
    }
    Ok(options)
  }
}

/// Progress reported while a bundle is downloading.
#[derive(uniffi::Record, Clone, Debug)]
pub struct RemoteOnDownloadData {
  /// Bytes downloaded so far.
  pub downloaded_bytes: u64,
  /// Total bytes to download, when the server advertised a content length.
  pub total_bytes: Option<u64>,
  /// The endpoint the bundle is being downloaded from.
  pub endpoint: String,
}

/// A callback invoked with download progress as a bundle downloads.
#[uniffi::export(with_foreign)]
pub trait RemoteOnDownload: Send + Sync {
  fn on_download(&self, data: RemoteOnDownloadData);
}

/// Options for creating a [`Remote`] client.
#[derive(uniffi::Record, Clone, Default)]
pub struct RemoteOptions {
  /// HTTP client options.
  #[uniffi(default = None)]
  pub http: Option<HttpOptions>,
  /// Download progress callback.
  #[uniffi(default = None)]
  pub on_download: Option<Arc<dyn RemoteOnDownload>>,
}

/// Options for fetching bundle metadata from the remote.
#[derive(uniffi::Record, Clone, Debug, Default)]
pub struct RemoteFetchOptions {
  /// Release channel (e.g. `"stable"`, `"beta"`). Passed as a query parameter to the remote.
  #[uniffi(default = None)]
  pub channel: Option<String>,
}

impl From<RemoteFetchOptions> for remote::RemoteFetchOptions {
  fn from(value: RemoteFetchOptions) -> Self {
    let mut options = remote::RemoteFetchOptions::default();
    if let Some(channel) = value.channel {
      options = options.channel(channel);
    }
    options
  }
}

/// Summary of a bundle returned by the remote listing endpoint.
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

/// Full metadata returned when fetching or downloading a specific bundle version.
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

/// Result of a bundle download containing the parsed bundle, its raw bytes,
/// and the server-provided metadata.
///
/// `data` holds the raw `.wvb` bytes as received from the server, which callers
/// can persist to disk via [`BundleSource::write_remote_bundle`].
#[derive(uniffi::Record, Clone, Debug)]
pub struct DownloadResult {
  pub info: RemoteBundleInfo,
  pub bundle: Arc<Bundle>,
  pub data: Vec<u8>,
}

/// HTTP client for a WebViewBundle remote server.
#[derive(uniffi::Object)]
pub struct Remote {
  pub(crate) inner: Arc<remote::Remote>,
}

#[uniffi::export]
impl Remote {
  /// Creates a client for the server at `endpoint` (e.g. `"https://bundles.example.com"`).
  #[uniffi::constructor(default(options = None))]
  pub fn new(
    endpoint: String,
    options: Option<RemoteOptions>,
  ) -> Result<Arc<Remote>, crate::Error> {
    let mut builder = remote::Remote::builder().endpoint(endpoint);
    if let Some(options) = options {
      if let Some(http) = options.http {
        builder = builder.http(remote::HttpOptions::try_from(http)?);
      }
      if let Some(on_download) = options.on_download {
        builder = builder.on_download(move |downloaded_bytes, total_bytes, endpoint| {
          on_download.on_download(RemoteOnDownloadData {
            downloaded_bytes,
            total_bytes,
            endpoint,
          });
        });
      }
    }
    let inner = builder.build()?;
    Ok(Arc::new(Remote {
      inner: Arc::new(inner),
    }))
  }
}

#[uniffi::export(async_runtime = "tokio")]
impl Remote {
  pub async fn list_bundles(
    &self,
    options: Option<RemoteFetchOptions>,
  ) -> Result<Vec<ListRemoteBundleInfo>, crate::Error> {
    let bundles = self
      .inner
      .list_bundles(options.map(Into::into))
      .await?
      .into_iter()
      .map(ListRemoteBundleInfo::from)
      .collect();
    Ok(bundles)
  }

  /// Fetches metadata for the latest version of `bundle_name` without downloading the bundle.
  pub async fn get_info(
    &self,
    bundle_name: String,
    options: Option<RemoteFetchOptions>,
  ) -> Result<RemoteBundleInfo, crate::Error> {
    let info = self
      .inner
      .get_current_info(&bundle_name, options.map(Into::into))
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
      data,
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
      data,
    })
  }
}
