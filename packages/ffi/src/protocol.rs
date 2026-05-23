use crate::http::{HttpMethod, HttpResponse, request};
use crate::source::BundleSource;
use std::collections::HashMap;
use std::sync::Arc;
use wvb::protocol;
use wvb::protocol::Protocol;

/// Handles HTTP-like requests by serving bundle entries from a [`BundleSource`].
///
/// The host portion of the URI identifies the bundle by name
/// (e.g. `https://app.wvb/index.html` → bundle `"app"`, path `"/index.html"`).
/// Returns 200 with the entry body, 404 when the path is not found, or
/// 200 with an empty body for HEAD requests.
#[derive(uniffi::Object)]
pub struct BundleUrlHandler {
  inner: Arc<protocol::BundleProtocol>,
}

#[uniffi::export]
impl BundleUrlHandler {
  #[uniffi::constructor]
  pub fn new(source: Arc<BundleSource>) -> Arc<BundleUrlHandler> {
    Arc::new(BundleUrlHandler {
      inner: Arc::new(protocol::BundleProtocol::new(source.inner.clone())),
    })
  }
}

#[uniffi::export(async_runtime = "tokio")]
impl BundleUrlHandler {
  pub async fn handle(
    &self,
    method: HttpMethod,
    uri: String,
    headers: Option<HashMap<String, String>>,
  ) -> Result<HttpResponse, crate::Error> {
    let req = request(method, uri, headers)?;
    let resp = self.inner.handle(req).await?;
    Ok(HttpResponse::from(resp))
  }
}

/// Proxies HTTP-like requests to a local HTTP server.
///
/// `hosts` maps virtual hostnames to local server base URLs
/// (e.g. `{"myapp" => "http://localhost:8080"}`). Requests to an unknown
/// host are returned as an error.
#[derive(uniffi::Object)]
pub struct LocalUrlHandler {
  inner: Arc<protocol::LocalProtocol>,
}

#[uniffi::export]
impl LocalUrlHandler {
  #[uniffi::constructor]
  pub fn new(hosts: HashMap<String, String>) -> Arc<LocalUrlHandler> {
    Arc::new(LocalUrlHandler {
      inner: Arc::new(protocol::LocalProtocol::new(hosts)),
    })
  }
}

#[uniffi::export(async_runtime = "tokio")]
impl LocalUrlHandler {
  pub async fn handle(
    &self,
    method: HttpMethod,
    uri: String,
    headers: Option<HashMap<String, String>>,
  ) -> Result<HttpResponse, crate::Error> {
    let req = request(method, uri, headers)?;
    let resp = self.inner.handle(req).await?;
    Ok(HttpResponse::from(resp))
  }
}
