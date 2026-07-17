use crate::remote::HttpOptions;
use std::sync::Arc;

pub(crate) type RemoteOnDownload = dyn Fn(u64, Option<u64>, String) + Send + Sync + 'static;

/// Options for remote operations.
#[derive(Default, Clone)]
#[non_exhaustive]
pub struct RemoteOptions {
  /// Base URL of the remote server where bundles are hosted.
  ///
  /// This URL is used as the prefix for all API endpoints. The client automatically
  /// appends API paths to construct full URLs for each operation.
  pub endpoint: String,
  /// Download progress callback.
  pub on_download: Option<Arc<RemoteOnDownload>>,
  /// Optional HTTP client options.
  pub http: Option<HttpOptions>,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub struct RemoteFetchOptions {
  pub channel: Option<String>,
}

impl RemoteFetchOptions {
  pub fn channel(mut self, channel: impl Into<String>) -> Self {
    self.channel = Some(channel.into());
    self
  }
}
