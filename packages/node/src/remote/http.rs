use napi_derive::napi;
use std::collections::HashMap;
use wvb::http::{HeaderMap, HeaderName, HeaderValue};

#[derive(Default)]
#[napi(object)]
pub struct HttpOptions {
  pub default_headers: Option<HashMap<String, String>>,
  pub user_agent: Option<String>,
  pub timeout: Option<u32>,
  pub read_timeout: Option<u32>,
  pub connect_timeout: Option<u32>,
  pub pool_idle_timeout: Option<u32>,
  pub pool_max_idle_per_host: Option<u32>,
  pub referer: Option<bool>,
  pub tcp_nodelay: Option<bool>,
}

impl TryFrom<HttpOptions> for wvb::remote::HttpOptions {
  type Error = crate::Error;
  fn try_from(value: HttpOptions) -> Result<Self, Self::Error> {
    let mut options = wvb::remote::HttpOptions::new();
    if let Some(default_headers) = value.default_headers {
      let mut headers = HeaderMap::with_capacity(default_headers.len());
      for (n, v) in default_headers {
        let name = HeaderName::from_bytes(n.as_bytes())?;
        let value = HeaderValue::from_bytes(v.as_bytes())?;
        headers.insert(name, value);
      }
      options = options.default_headers(headers);
    }
    if let Some(user_agent) = value.user_agent {
      options = options.user_agent(user_agent);
    }
    if let Some(timeout) = value.timeout {
      options = options.timeout(timeout as u64);
    }
    if let Some(read_timeout) = value.read_timeout {
      options = options.read_timeout(read_timeout as u64);
    }
    if let Some(connect_timeout) = value.connect_timeout {
      options = options.connect_timeout(connect_timeout as u64);
    }
    if let Some(pool_idle_timeout) = value.pool_idle_timeout {
      options = options.pool_idle_timeout(pool_idle_timeout as u64);
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
