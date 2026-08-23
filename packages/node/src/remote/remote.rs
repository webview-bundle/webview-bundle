use crate::cancellation::Cancellation;
use crate::js::{JsCallback, JsCallbackExt};
use crate::remote::HttpOptions;
use crate::signature::SignatureVerifyKey;
use napi::Env;
use napi_derive::napi;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use wvb::remote;

#[napi(object)]
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

#[napi(object)]
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

#[napi(object)]
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

#[napi(object)]
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

/// Download progress data.
///
/// @property {number} downloadedBytes - Bytes downloaded so far
/// @property {number} totalBytes - Total bytes to download
/// @property {string} url - URL being downloaded from
#[napi(object)]
pub struct RemoteOnDownloadData {
  pub downloaded_bytes: u32,
  pub total_bytes: Option<u32>,
  pub url: String,
}

#[napi(object, object_to_js = false)]
pub struct RemoteConfig {
  pub base_url: String,
  pub http: Option<HttpOptions>,
  #[napi(ts_type = "(data: RemoteOnDownloadData) => void")]
  pub on_download: Option<JsCallback<RemoteOnDownloadData, ()>>,
}

#[napi(object, object_to_js = false)]
pub struct RemoteGetUpdateOptions {
  pub etag: Option<String>,
  pub channel: Option<String>,
  pub expect_signature: Option<SignatureVerifyKey>,
}

impl From<RemoteGetUpdateOptions> for remote::RemoteGetUpdateOptions {
  fn from(value: RemoteGetUpdateOptions) -> Self {
    let mut options = remote::RemoteGetUpdateOptions::default();
    if let Some(etag) = value.etag {
      options = options.etag(etag);
    }
    if let Some(channel) = value.channel {
      options = options.channel(channel);
    }
    if let Some(expect_signature) = value.expect_signature {
      options = options.expect_signature(expect_signature.into());
    }
    options
  }
}

/// HTTP client for getting updates and downloading bundles from a remote server.
///
/// @example
/// ```typescript
/// const remote = new Remote({ baseUrl: 'https://updates.example.com' });
///
/// const response = await remote.getUpdate();
/// if (response != null) {
///   for (const bundle of response.update.bundles) {
///     await remote.download(bundle.downloadUrl, `./remote/${bundle.name}/${bundle.version}.wvb`);
///   }
/// }
/// ```
#[napi]
pub struct Remote {
  pub(crate) inner: Arc<remote::Remote>,
}

#[napi]
impl Remote {
  /// Creates a new remote client.
  ///
  /// @param {RemoteConfig} config - Client config
  ///
  /// @example
  /// ```typescript
  /// const remote = new Remote({
  ///   baseUrl: 'https://updates.example.com',
  ///   http: { timeout: 60000 },
  ///   onDownload: data => {
  ///     const percent = (data.downloadedBytes / data.totalBytes) * 100;
  ///     console.log(`Progress: ${percent.toFixed(1)}%`);
  ///   },
  /// });
  /// ```
  #[napi(constructor)]
  pub fn new(env: Env, config: RemoteConfig) -> napi::Result<Remote> {
    crate::Outcome::from_fn(|| {
      let mut builder = remote::Remote::builder().base_url(config.base_url);
      if let Some(http) = config.http {
        builder = builder.http(remote::HttpOptions::try_from(http)?);
      }
      if let Some(on_download) = config.on_download {
        builder = builder.on_download(move |downloaded_bytes, total_bytes, url| {
          let on_download_fn = Arc::clone(&on_download);
          let _ = on_download_fn.fire_and_forgot(RemoteOnDownloadData {
            downloaded_bytes: downloaded_bytes as u32,
            total_bytes: total_bytes.map(|t| t as u32),
            url,
          });
        });
      }
      let inner = builder.build()?;
      Ok(Remote {
        inner: Arc::new(inner),
      })
    })
    .into_napi(env)
  }

  /// Gets update information from the remote server.
  ///
  /// @param {RemoteGetUpdateOptions} [options] - Request options
  /// @returns {Promise<RemoteUpdateResponse | null>} Update response, or null when not modified
  #[napi(ts_return_type = "Promise<RemoteUpdateResponse | null>")]
  pub async fn get_update(
    &self,
    options: Option<RemoteGetUpdateOptions>,
  ) -> crate::Outcome<Option<RemoteUpdateResponse>> {
    crate::Outcome::from_future(async {
      let update = self
        .inner
        .get_update(options.map(Into::into))
        .await?
        .map(RemoteUpdateResponse::from);
      Ok(update)
    })
    .await
  }

  /// Downloads a bundle into the given file path.
  ///
  /// @param {string} url - URL to download from
  /// @param {string} filepath - Destination file path
  /// @param {Cancellation} [cancellation] - Cancels the download when triggered
  #[napi(ts_return_type = "Promise<void>")]
  pub async fn download(
    &self,
    url: String,
    filepath: String,
    cancellation: Option<&Cancellation>,
  ) -> crate::Outcome<()> {
    crate::Outcome::from_future(async {
      self
        .inner
        .download(
          url,
          Path::new(&filepath),
          cancellation.map(|x| x.inner.clone()),
        )
        .await?;
      Ok(())
    })
    .await
  }
}
