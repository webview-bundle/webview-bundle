use crate::bundle::Bundle;
use crate::js::{JsCallback, JsCallbackExt};
use crate::remote::HttpOptions;
use napi::Status;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::sync::Arc;
use wvb::remote;
use wvb::remote::HttpConfig;

/// Options for creating a remote client.
///
/// @property {HttpOptions} [http] - HTTP client configuration
/// @property {(data: RemoteOnDownloadData) => void} [onDownload] - Download progress callback
///
/// @example
/// ```typescript
/// const options = {
///   http: { timeout: 30000 },
///   onDownload: (data) => {
///     console.log(`Downloaded ${data.downloadedBytes}/${data.totalBytes}`);
///   }
/// };
/// const remote = new Remote("https://updates.example.com", options);
/// ```
#[napi(object, object_to_js = false)]
pub struct RemoteOptions {
  pub http: Option<HttpOptions>,
  #[napi(ts_type = "(data: RemoteOnDownloadData) => void")]
  pub on_download: Option<JsCallback<RemoteOnDownloadData, ()>>,
}

/// Download progress data.
///
/// @property {number} downloadedBytes - Bytes downloaded so far
/// @property {number} totalBytes - Total bytes to download
/// @property {string} endpoint - Endpoint being downloaded from
#[napi(object)]
pub struct RemoteOnDownloadData {
  pub downloaded_bytes: u32,
  pub total_bytes: u32,
  pub endpoint: String,
}

/// Bundle information from list operations.
///
/// @property {string} name - Bundle name
/// @property {string} version - Version string
#[napi(object)]
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

/// Complete bundle information from remote server.
///
/// Contains version, cache validation, and integrity data.
///
/// @property {string} name - Bundle name
/// @property {string} version - Version string
/// @property {string} [etag] - HTTP ETag for cache validation
/// @property {string} [integrity] - SHA3 integrity hash
/// @property {string} [signature] - Digital signature
/// @property {string} [lastModified] - Last-Modified timestamp
#[napi(object)]
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

/// HTTP client for downloading bundles from a remote server.
///
/// The remote client implements the bundle HTTP protocol, allowing you to:
/// - List available bundles
/// - Get bundle metadata
/// - Download specific versions
/// - Track download progress
///
/// @example
/// ```typescript
/// const remote = new Remote("https://updates.example.com");
///
/// // List all bundles
/// const bundles = await remote.listBundles();
///
/// // Get current version info
/// const info = await remote.getInfo("app");
/// console.log(`Latest version: ${info.version}`);
///
/// // Download bundle
/// const [bundleInfo, bundle, data] = await remote.download("app");
/// ```
#[napi]
pub struct Remote {
  pub(crate) inner: Arc<remote::Remote>,
}

#[napi]
impl Remote {
  /// Creates a new remote client.
  ///
  /// @param {string} endpoint - Base URL of the remote server
  /// @param {RemoteOptions} [options] - Client options
  ///
  /// @example
  /// ```typescript
  /// const remote = new Remote("https://updates.example.com");
  /// ```
  ///
  /// @example
  /// ```typescript
  /// // With options
  /// const remote = new Remote("https://updates.example.com", {
  ///   http: { timeout: 60000 },
  ///   onDownload: (data) => {
  ///     const percent = (data.downloadedBytes / data.totalBytes) * 100;
  ///     console.log(`Progress: ${percent.toFixed(1)}%`);
  ///   }
  /// });
  /// ```
  #[napi(constructor)]
  pub fn new(endpoint: String, options: Option<RemoteOptions>) -> crate::Result<Remote> {
    let mut builder = remote::Remote::builder().endpoint(endpoint);
    if let Some(options) = options {
      if let Some(http) = options.http {
        builder = builder.http(
          HttpConfig::try_from(http).map_err(|e| Error::new(Status::InvalidArg, e.to_string()))?,
        );
      }
      if let Some(on_download) = options.on_download {
        builder = builder.on_download(move |downloaded_bytes, total_bytes, endpoint| {
          let on_download_fn = Arc::clone(&on_download);
          let _ = on_download_fn.invoke_sync(RemoteOnDownloadData {
            downloaded_bytes: downloaded_bytes as u32,
            total_bytes: total_bytes as u32,
            endpoint,
          });
        });
      }
    }
    let inner = builder.build()?;
    Ok(Remote {
      inner: Arc::new(inner),
    })
  }

  /// Lists all available bundles on the server.
  ///
  /// @param {string} [channel] - Optional channel filter
  /// @returns {Promise<ListRemoteBundleInfo[]>} List of bundles
  ///
  /// @example
  /// ```typescript
  /// const bundles = await remote.listBundles();
  /// for (const bundle of bundles) {
  ///   console.log(`${bundle.name}@${bundle.version}`);
  /// }
  /// ```
  #[napi]
  pub async fn list_bundles(
    &self,
    channel: Option<String>,
  ) -> crate::Result<Vec<ListRemoteBundleInfo>> {
    let bundles = self
      .inner
      .list_bundles(channel.as_ref())
      .await?
      .into_iter()
      .map(ListRemoteBundleInfo::from)
      .collect::<Vec<_>>();
    Ok(bundles)
  }

  /// Gets bundle metadata for the current version.
  ///
  /// Fetches metadata without downloading the bundle itself.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @param {string} [channel] - Optional channel filter
  /// @returns {Promise<RemoteBundleInfo>} Bundle information
  ///
  /// @example
  /// ```typescript
  /// const info = await remote.getInfo("app");
  /// console.log(`Current version: ${info.version}`);
  /// if (info.integrity) {
  ///   console.log(`Integrity: ${info.integrity}`);
  /// }
  /// ```
  #[napi]
  pub async fn get_info(
    &self,
    bundle_name: String,
    channel: Option<String>,
  ) -> crate::Result<RemoteBundleInfo> {
    let info = self
      .inner
      .get_current_info(&bundle_name, channel.as_ref())
      .await?;
    Ok(info.into())
  }

  /// Downloads the current version of a bundle.
  ///
  /// Returns bundle info, parsed bundle, and raw data.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @param {string} [channel] - Optional channel filter
  /// @returns {Promise<[RemoteBundleInfo, Bundle, Buffer]>} Tuple of info, bundle, and data
  ///
  /// @example
  /// ```typescript
  /// const [info, bundle, data] = await remote.download("app");
  /// console.log(`Downloaded ${info.name}@${info.version}`);
  /// console.log(`Size: ${data.length} bytes`);
  ///
  /// // Save to file
  /// await writeBundle(bundle, "app.wvb");
  /// ```
  #[napi]
  pub async fn download(
    &self,
    bundle_name: String,
    channel: Option<String>,
  ) -> crate::Result<(RemoteBundleInfo, Bundle, Buffer)> {
    let (info, inner, data) = self.inner.download(&bundle_name, channel.as_ref()).await?;
    Ok((info.into(), Bundle { inner }, data.into()))
  }

  /// Downloads a specific version of a bundle.
  ///
  /// @param {string} bundleName - Name of the bundle
  /// @param {string} version - Specific version to download
  /// @returns {Promise<[RemoteBundleInfo, Bundle, Buffer]>} Tuple of info, bundle, and data
  ///
  /// @example
  /// ```typescript
  /// const [info, bundle, data] = await remote.downloadVersion("app", "1.0.0");
  /// console.log(`Downloaded specific version: ${info.version}`);
  /// ```
  #[napi]
  pub async fn download_version(
    &self,
    bundle_name: String,
    version: String,
  ) -> crate::Result<(RemoteBundleInfo, Bundle, Buffer)> {
    let (info, inner, data) = self.inner.download_version(&bundle_name, &version).await?;
    Ok((info.into(), Bundle { inner }, data.into()))
  }
}
