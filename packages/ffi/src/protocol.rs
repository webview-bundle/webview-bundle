use crate::http::{HttpMethod, HttpResponse, request};
use crate::source::BundleSource;
use std::collections::HashMap;
use std::sync::Arc;
use wvb::protocol;
use wvb::protocol::Protocol;

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
    let resp = self.inner.handle(req).await.map_err(wvb::Error::from)?;
    Ok(HttpResponse::from(resp))
  }
}

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
    let resp = self.inner.handle(req).await.map_err(wvb::Error::from)?;
    Ok(HttpResponse::from(resp))
  }
}
