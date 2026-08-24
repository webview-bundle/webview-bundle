use crate::remote::HttpOptions;
use std::sync::Arc;

pub type RemoteOnDownload = dyn Fn(u64, Option<u64>, String) + Send + Sync + 'static;

/// Config value for remote
#[derive(Default, Clone)]
#[non_exhaustive]
pub struct RemoteConfig {
  /// Base URL of the remote server
  pub base_url: String,
  /// Download progress callback
  pub on_download: Option<Arc<RemoteOnDownload>>,
  /// Optional HTTP client options
  pub http: Option<HttpOptions>,
}
