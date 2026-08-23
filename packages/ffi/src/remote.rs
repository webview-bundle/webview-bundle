use crate::cancellation::Cancellation;
use crate::http::HttpHeaders;
use crate::signature::SignatureVerifyKey;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use wvb::remote;

#[derive(uniffi::Record, Clone, Debug)]
pub struct BundleUpdate {
  pub name: String,
  pub version: String,
  pub download_url: Option<String>,
  pub integrity: Option<String>,
  pub metadata: Option<HashMap<String, String>>,
}

impl From<remote::BundleUpdate> for BundleUpdate {
  fn from(value: remote::BundleUpdate) -> Self {
    Self {
      name: value.name,
      version: value.version,
      download_url: value.download_url,
      integrity: value.integrity,
      metadata: value.metadata,
    }
  }
}

impl From<BundleUpdate> for remote::BundleUpdate {
  fn from(value: BundleUpdate) -> Self {
    Self {
      name: value.name,
      version: value.version,
      download_url: value.download_url,
      integrity: value.integrity,
      metadata: value.metadata,
    }
  }
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct Update {
  pub id: String,
  pub created_at: String,
  pub runtime_version: u8,
  pub bundles: Vec<BundleUpdate>,
  pub metadata: HashMap<String, String>,
}

impl From<remote::Update> for Update {
  fn from(value: remote::Update) -> Self {
    Self {
      id: value.id,
      created_at: value.created_at,
      runtime_version: value.runtime_version,
      bundles: value
        .bundles
        .into_iter()
        .map(BundleUpdate::from)
        .collect::<Vec<_>>(),
      metadata: value.metadata,
    }
  }
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct UpdateSignature {
  pub key_id: String,
  pub sig: String,
  pub alg: String,
}

impl From<remote::UpdateSignature> for UpdateSignature {
  fn from(value: remote::UpdateSignature) -> Self {
    Self {
      key_id: value.key_id,
      sig: value.sig,
      alg: value.alg,
    }
  }
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct RemoteUpdateResponse {
  pub update: Update,
  pub etag: Option<String>,
  pub signature: Option<UpdateSignature>,
}

impl From<remote::RemoteUpdateResponse> for RemoteUpdateResponse {
  fn from(value: remote::RemoteUpdateResponse) -> Self {
    Self {
      update: value.update.into(),
      etag: value.etag,
      signature: value.signature.map(Into::into),
    }
  }
}

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
      options = options.default_headers(wvb::http::HeaderMap::try_from(HttpHeaders::from(
        default_headers,
      ))?);
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
  pub url: String,
}

/// A callback invoked with download progress as a bundle downloads.
#[uniffi::export(with_foreign)]
pub trait RemoteOnDownload: Send + Sync {
  fn on_download(&self, data: RemoteOnDownloadData);
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct RemoteGetUpdateOptions {
  #[uniffi(default = None)]
  pub etag: Option<String>,
  #[uniffi(default = None)]
  pub channel: Option<String>,
  #[uniffi(default = None)]
  pub expect_signature: Option<SignatureVerifyKey>,
}

impl TryFrom<RemoteGetUpdateOptions> for remote::RemoteGetUpdateOptions {
  type Error = crate::Error;

  fn try_from(value: RemoteGetUpdateOptions) -> Result<Self, Self::Error> {
    let mut options = remote::RemoteGetUpdateOptions::default();
    if let Some(etag) = value.etag {
      options = options.etag(etag);
    }
    if let Some(channel) = value.channel {
      options = options.channel(channel);
    }
    if let Some(expect_signature) = value.expect_signature {
      options = options.expect_signature(expect_signature.try_into()?);
    }
    Ok(options)
  }
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

/// HTTP client for a WebViewBundle remote server.
#[derive(uniffi::Object)]
pub struct Remote {
  pub(crate) inner: Arc<remote::Remote>,
}

#[uniffi::export]
impl Remote {
  /// Creates a client for the server at `base_url` (e.g. `"https://bundles.example.com"`).
  #[uniffi::constructor(default(options = None))]
  pub fn new(
    base_url: String,
    options: Option<RemoteOptions>,
  ) -> Result<Arc<Remote>, crate::Error> {
    let mut builder = remote::Remote::builder().base_url(base_url);
    if let Some(options) = options {
      if let Some(http) = options.http {
        builder = builder.http(remote::HttpOptions::try_from(http)?);
      }
      if let Some(on_download) = options.on_download {
        builder = builder.on_download(move |downloaded_bytes, total_bytes, url| {
          on_download.on_download(RemoteOnDownloadData {
            downloaded_bytes,
            total_bytes,
            url,
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
  pub async fn get_update(
    &self,
    options: Option<RemoteGetUpdateOptions>,
  ) -> crate::Result<Option<RemoteUpdateResponse>> {
    let resp = self
      .inner
      .get_update(options.map(TryInto::try_into).transpose()?)
      .await?
      .map(RemoteUpdateResponse::from);
    Ok(resp)
  }

  #[uniffi::method(default(cancellation = None))]
  pub async fn download(
    &self,
    url: String,
    filepath: String,
    cancellation: Option<Arc<Cancellation>>,
  ) -> crate::Result<()> {
    self
      .inner
      .download(
        url,
        Path::new(&filepath),
        cancellation.map(|x| x.inner.clone()),
      )
      .await?;
    Ok(())
  }
}
