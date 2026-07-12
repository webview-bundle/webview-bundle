use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Deref;
use wvb::http;
use wvb::http::HeaderMap;

/// HTTP request method exposed to FFI consumers.
#[derive(uniffi::Enum, Clone, Debug)]
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

/// Newtype wrapper for converting between `HashMap<String, String>` and
/// `http::HeaderMap`. Not exposed to FFI; used internally for conversions.
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

/// HTTP response returned by protocol handlers.
#[derive(uniffi::Record, Clone, Debug)]
pub struct HttpResponse {
  pub status: u16,
  pub headers: HashMap<String, String>,
  pub body: Vec<u8>,
}

impl From<http::Response<Cow<'static, [u8]>>> for HttpResponse {
  fn from(value: http::Response<Cow<'static, [u8]>>) -> Self {
    let status = value.status().as_u16();
    let headers = HttpHeaders::from(value.headers()).0;
    let body = value.body().to_vec();
    HttpResponse {
      status,
      headers,
      body,
    }
  }
}

/// Builds an `http::Request` from FFI-friendly arguments.
/// Used internally by protocol handler `handle` methods.
pub(crate) fn request(
  method: HttpMethod,
  uri: String,
  headers: Option<HashMap<String, String>>,
  body: Option<Vec<u8>>,
) -> Result<http::Request<Vec<u8>>, crate::Error> {
  let mut req = http::Request::builder()
    .method(http::Method::from(method))
    .uri(&uri);
  if let Some(headers) = headers {
    for (key, value) in headers {
      req = req.header(key, value);
    }
  }
  let req = req
    .body(body.unwrap_or_default())
    .map_err(|e| crate::Error::from(wvb::Error::from(e)))?;
  Ok(req)
}
