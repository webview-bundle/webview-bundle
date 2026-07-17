use crate::remote::{
  HttpOptions, ListRemoteBundleInfo, RemoteBundleInfo, RemoteError, RemoteFetchOptions,
  RemoteOptions,
};
use crate::{Bundle, BundleReader, Reader};
use futures_util::StreamExt;
use http::{StatusCode, header, uri::Uri};
use std::io::Cursor;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Default, Clone)]
pub struct RemoteBuilder {
  options: RemoteOptions,
}

impl RemoteBuilder {
  #[must_use]
  /// Set the base url of the remote server where bundles are hosted.
  pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
    self.options.endpoint = endpoint.into();
    self
  }

  /// Set HTTP client options.
  pub fn http(mut self, http: HttpOptions) -> Self {
    self.options.http = Some(http);
    self
  }

  /// Set download progress callback.
  pub fn on_download<F>(mut self, on_download: F) -> Self
  where
    F: Fn(u64, Option<u64>, String) + Send + Sync + 'static,
  {
    self.options.on_download = Some(Arc::new(on_download));
    self
  }

  /// Build the remote client with the configured options.
  pub fn build(self) -> crate::Result<Remote> {
    if self.options.endpoint.is_empty() {
      return Err(crate::Error::invalid_remote_config("endpoint is empty"));
    }
    // Apply HTTP options unconditionally (defaulting when unset) so the default request
    // timeout is always in effect, even when the caller passes no `HttpOptions`.
    let http_options = self.options.http.clone().unwrap_or_default();
    let client_builder = http_options.apply(reqwest::ClientBuilder::new());
    let client = client_builder.build()?;
    Ok(Remote {
      options: self.options,
      client,
    })
  }
}

/// Remote client for using with remote bundles.
#[derive(Clone)]
pub struct Remote {
  options: RemoteOptions,
  client: reqwest::Client,
}

impl Remote {
  pub fn builder() -> RemoteBuilder {
    RemoteBuilder::default()
  }

  /// `GET /bundles` : List bundles from remote
  pub async fn list_bundles(
    &self,
    options: Option<RemoteFetchOptions>,
  ) -> crate::Result<Vec<ListRemoteBundleInfo>> {
    let endpoint = self.endpoint(
      "/bundles",
      options
        .and_then(|x| x.channel)
        .map(|x| vec![("channel", x)]),
    )?;
    let resp = self.client.get(endpoint).send().await?;
    match resp.status().is_success() {
      true => Ok(resp.json::<Vec<ListRemoteBundleInfo>>().await?),
      false => Err(self.parse_err(resp).await),
    }
  }

  /// `HEAD /bundles/:name` : Get bundle metadata info
  pub async fn get_current_info(
    &self,
    bundle_name: &str,
    options: Option<RemoteFetchOptions>,
  ) -> crate::Result<RemoteBundleInfo> {
    let endpoint = self.endpoint(
      format!("/bundles/{bundle_name}"),
      options
        .and_then(|x| x.channel)
        .map(|x| vec![("channel", x)]),
    )?;
    let resp = self.client.head(endpoint).send().await?;
    match resp.status().is_success() {
      true => Ok(self.parse_info(&resp)?),
      false => Err(self.parse_err(resp).await),
    }
  }

  /// GET /bundles/:name
  pub async fn download(
    &self,
    bundle_name: &str,
    channel: Option<&String>,
  ) -> crate::Result<(RemoteBundleInfo, Bundle, Vec<u8>)> {
    self
      .download_inner(format!("/bundles/{bundle_name}"), channel)
      .await
  }

  /// GET /bundles/:name/:version
  pub async fn download_version(
    &self,
    bundle_name: &str,
    version: &str,
  ) -> crate::Result<(RemoteBundleInfo, Bundle, Vec<u8>)> {
    self
      .download_inner(format!("/bundles/{bundle_name}/{version}"), None)
      .await
  }

  fn endpoint(
    &self,
    path: impl Into<String>,
    query: Option<Vec<(impl Into<String>, impl Into<String>)>>,
  ) -> crate::Result<String> {
    let endpoint = self
      .options
      .endpoint
      .strip_suffix('/')
      .unwrap_or(&self.options.endpoint);
    let p = path.into().trim_matches('/').to_string();
    let q = query
      .map(|x| {
        x.into_iter()
          .map(|(k, v)| {
            format!(
              "{}={}",
              urlencoding::encode(&k.into()),
              urlencoding::encode(&v.into())
            )
          })
          .collect::<Vec<_>>()
          .join("&")
      })
      .map(|qs| format!("?{}", qs))
      .unwrap_or_default();
    let input = format!("{}/{}{}", endpoint, p, q);
    let uri = Uri::from_str(&input).map_err(crate::Error::InvalidRemoteUrl)?;
    Ok(uri.to_string())
  }

  fn parse_info(&self, resp: &reqwest::Response) -> crate::Result<RemoteBundleInfo> {
    let headers = resp.headers();
    let name = get_header_value(headers, "webview-bundle-name").ok_or(
      crate::Error::invalid_remote_bundle("\"webview-bundle-name\" header is missing"),
    )?;
    let version = get_header_value(headers, "webview-bundle-version").ok_or(
      crate::Error::invalid_remote_bundle("\"webview-bundle-version\" header is missing"),
    )?;
    let etag = get_header_value(headers, header::ETAG);
    let last_modified = get_header_value(headers, header::LAST_MODIFIED);
    let integrity = get_header_value(headers, "webview-bundle-integrity");
    let signature = get_header_value(headers, "webview-bundle-signature");
    Ok(RemoteBundleInfo {
      name,
      version,
      etag,
      integrity,
      signature,
      last_modified,
    })
  }

  async fn parse_err(&self, resp: reqwest::Response) -> crate::Error {
    let status = resp.status();
    if status == StatusCode::FORBIDDEN {
      return crate::Error::RemoteForbidden;
    } else if status == StatusCode::NOT_FOUND {
      return crate::Error::RemoteBundleNotFound;
    }
    let message = resp
      .json::<RemoteError>()
      .await
      .map(|x| x.message)
      .unwrap_or_default();
    crate::Error::remote_http(status, message)
  }

  async fn download_inner(
    &self,
    path: String,
    channel: Option<&String>,
  ) -> crate::Result<(RemoteBundleInfo, Bundle, Vec<u8>)> {
    let endpoint = self.endpoint(path, channel.map(|x| vec![("channel", x)]))?;
    let resp = self.client.get(&endpoint).send().await?;
    if !resp.status().is_success() {
      return Err(self.parse_err(resp).await);
    }
    let info = self.parse_info(&resp)?;
    let total_size = resp.content_length();
    let mut stream = resp.bytes_stream();
    let mut downloaded_bytes: u64 = 0;
    let mut data = match total_size {
      Some(size) => Vec::with_capacity(size as usize),
      None => Vec::new(),
    };
    while let Some(chunk_result) = stream.next().await {
      let chunk = chunk_result?;
      data.append(&mut chunk.to_vec());
      downloaded_bytes += chunk.len() as u64;
      if let Some(on_download) = &self.options.on_download {
        on_download(downloaded_bytes, total_size, endpoint.to_owned());
      }
    }
    let mut reader = Cursor::new(&data);
    let bundle = Reader::<Bundle>::read(&mut BundleReader::new(&mut reader))?;
    Ok((info, bundle, data))
  }
}

fn get_header_value<K>(headers: &header::HeaderMap, key: K) -> Option<String>
where
  K: header::AsHeaderName,
{
  headers
    .get(key)
    .map(|x| String::from_utf8_lossy(x.as_bytes()).to_string())
}
