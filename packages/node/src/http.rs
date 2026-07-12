use napi::bindgen_prelude::Buffer;
use napi_derive::napi;
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Deref;
use wvb::http;
use wvb::http::HeaderMap;

#[napi(string_enum = "lowercase")]
pub enum HttpMethod {
  Get,
  Head,
  Options,
  Post,
  Put,
  Patch,
  Delete,
  Trace,
  Connect,
}

impl From<HttpMethod> for http::Method {
  fn from(method: HttpMethod) -> Self {
    match method {
      HttpMethod::Get => Self::GET,
      HttpMethod::Head => Self::HEAD,
      HttpMethod::Options => Self::OPTIONS,
      HttpMethod::Post => Self::POST,
      HttpMethod::Put => Self::PUT,
      HttpMethod::Patch => Self::PATCH,
      HttpMethod::Delete => Self::DELETE,
      HttpMethod::Trace => Self::TRACE,
      HttpMethod::Connect => Self::CONNECT,
    }
  }
}

pub struct HttpHeaders(pub HashMap<String, String>);

impl Deref for HttpHeaders {
  type Target = HashMap<String, String>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<HashMap<String, String>> for HttpHeaders {
  fn from(value: HashMap<String, String>) -> Self {
    Self(value)
  }
}

impl TryFrom<HttpHeaders> for HeaderMap {
  type Error = crate::Error;
  fn try_from(value: HttpHeaders) -> Result<Self, Self::Error> {
    let mut headers = HeaderMap::with_capacity(value.len());
    for (n, v) in value.0 {
      let name = http::HeaderName::from_bytes(n.as_bytes())?;
      let value = http::HeaderValue::from_bytes(v.as_bytes())?;
      headers.insert(name, value);
    }
    Ok(headers)
  }
}

impl From<&HeaderMap> for HttpHeaders {
  fn from(value: &HeaderMap) -> Self {
    Self(
      value
        .iter()
        .map(|(k, v)| {
          let value = String::from_utf8_lossy(v.as_ref()).to_string();
          (k.to_string(), value)
        })
        .collect::<HashMap<_, _>>(),
    )
  }
}

#[napi(object)]
pub struct HttpResponse {
  pub status: u16,
  pub headers: HashMap<String, String>,
  pub body: Buffer,
}

impl From<http::Response<Cow<'static, [u8]>>> for HttpResponse {
  fn from(value: http::Response<Cow<'static, [u8]>>) -> Self {
    let status = value.status().as_u16();
    let headers = HttpHeaders::from(value.headers()).0;
    let body = Buffer::from(value.body().as_ref());
    HttpResponse {
      status,
      headers,
      body,
    }
  }
}

pub(crate) fn request(
  method: HttpMethod,
  uri: String,
  headers: Option<HashMap<String, String>>,
  body: Option<Buffer>,
) -> crate::Result<http::Request<Vec<u8>>> {
  let mut req = http::Request::builder()
    .method(http::Method::from(method))
    .uri(&uri);
  if let Some(headers) = headers {
    for (key, value) in headers {
      req = req.header(key, value);
    }
  }
  let req = req
    .body(body.map(|body| body.to_vec()).unwrap_or_default())
    .map_err(|e| crate::Error::Core(wvb::Error::from(e)))?;
  Ok(req)
}
