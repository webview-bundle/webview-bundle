use async_trait::async_trait;
use dashmap::DashMap;
use http;
use http::Uri;
use std::borrow::Cow;
use std::collections::HashMap;

/// Trait for resolving custom URIs to localhost URLs.
///
/// Implement this trait to customize how protocol URIs are mapped to
/// localhost development servers.
pub trait LocalUriResolver: Send + Sync {
  /// Resolves a custom URI host to a localhost URL.
  ///
  /// # Arguments
  ///
  /// * `uri` - The original request URI
  ///
  /// # Returns
  ///
  /// The localhost URL (e.g., "http://localhost:3000") or None if not resolvable.
  fn resolve_localhost(&self, uri: &Uri) -> Option<String>;

  /// Gets the complete proxied localhost URI including path and query.
  ///
  /// This method combines the localhost URL from `resolve_localhost` with
  /// the path and query from the original URI.
  ///
  /// # Example
  ///
  /// ```text
  /// Input:  app://myapp/api/data?foo=bar
  /// Output: http://localhost:3000/api/data?foo=bar
  /// ```
  fn get_localhost_uri(&self, uri: &Uri) -> Option<String> {
    if let Some(localhost) = self.resolve_localhost(uri) {
      let decoded_path = percent_encoding::percent_decode(uri.path().as_bytes())
        .decode_utf8_lossy()
        .to_string();
      // Append the path of the original URI to the localhost URI.
      let url = format!(
        "{}/{}{}",
        localhost.trim_end_matches('/'),
        decoded_path.trim_start_matches('/'),
        match uri.query() {
          Some(q) => format!("?{q}"),
          None => "".to_string(),
        },
      );
      return Some(url);
    }
    None
  }
}

/// URI resolver using a static host-to-localhost mapping.
///
/// Maps custom protocol hosts to localhost URLs using a HashMap.
///
/// # Example
///
/// ```
/// # #[cfg(feature = "protocol-local")]
/// # {
/// use wvb::protocol::MappingLocalUriResolver;
/// use std::collections::HashMap;
///
/// let mut hosts = HashMap::new();
/// hosts.insert("myapp".to_string(), "http://localhost:3000".to_string());
/// hosts.insert("api".to_string(), "http://localhost:8080".to_string());
///
/// let resolver = MappingLocalUriResolver::new(hosts);
/// # }
/// ```
#[derive(Default)]
pub struct MappingLocalUriResolver {
  mapping: HashMap<String, String>,
}

impl MappingLocalUriResolver {
  /// Creates a new resolver with the given host mapping.
  pub fn new<T: Into<HashMap<String, String>>>(mapping: T) -> Self {
    Self {
      mapping: mapping.into(),
    }
  }
}

impl LocalUriResolver for MappingLocalUriResolver {
  fn resolve_localhost(&self, uri: &Uri) -> Option<String> {
    match uri.host() {
      Some(host) => self.mapping.get(host).cloned(),
      None => None,
    }
  }
}

#[derive(Clone)]
struct CachedResponse {
  status: http::StatusCode,
  headers: http::HeaderMap,
  body: bytes::Bytes,
}

/// Protocol handler that proxies requests to localhost servers.
///
/// `LocalProtocol` forwards requests to local development servers, making it
/// easy to develop webview applications with hot-reloading. Features:
///
/// - **Host mapping**: Map custom protocol hosts to localhost URLs
/// - **Response caching**: Cache responses and respect 304 Not Modified
/// - **Development mode**: Perfect for local development workflows
///
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "protocol-local")]
/// # async {
/// use wvb::protocol::{LocalProtocol, Protocol};
/// use std::collections::HashMap;
///
/// let mut hosts = HashMap::new();
/// hosts.insert("myapp".to_string(), "http://localhost:3000".to_string());
///
/// let protocol = LocalProtocol::new(hosts);
///
/// // This proxies to http://localhost:3000/index.html
/// let request = http::Request::builder()
///     .uri("app://myapp/index.html")
///     .body(vec![])
///     .unwrap();
///
/// let response = protocol.handle(request).await.unwrap();
/// # };
/// ```
///
/// # Caching
///
/// The protocol caches responses and supports HTTP 304 Not Modified:
///
/// ```no_run
/// # #[cfg(feature = "protocol-local")]
/// # async {
/// # use wvb::protocol::{LocalProtocol, Protocol};
/// # use std::collections::HashMap;
/// # let protocol = LocalProtocol::new(HashMap::new());
/// // First request - fetches from server
/// let request1 = http::Request::builder()
///     .uri("app://myapp/bundle.js")
///     .body(vec![])
///     .unwrap();
/// let response1 = protocol.handle(request1).await.unwrap();
///
/// // Second request with same URL - may use cached response
/// let request2 = http::Request::builder()
///     .uri("app://myapp/bundle.js")
///     .body(vec![])
///     .unwrap();
/// let response2 = protocol.handle(request2).await.unwrap();
/// # };
/// ```
pub struct LocalProtocol {
  uri_resolver: Box<dyn LocalUriResolver + 'static>,
  cache: DashMap<String, CachedResponse>,
}

impl LocalProtocol {
  /// Creates a new `LocalProtocol` with host-to-localhost mapping.
  ///
  /// # Arguments
  ///
  /// * `hosts` - HashMap mapping custom hosts to localhost URLs
  ///
  /// # Example
  ///
  /// ```
  /// # #[cfg(feature = "protocol-local")]
  /// # {
  /// use wvb::protocol::LocalProtocol;
  /// use std::collections::HashMap;
  ///
  /// let mut hosts = HashMap::new();
  /// hosts.insert("myapp".to_string(), "http://localhost:3000".to_string());
  /// hosts.insert("api".to_string(), "http://localhost:8080".to_string());
  ///
  /// let protocol = LocalProtocol::new(hosts);
  /// # }
  /// ```
  pub fn new<T: Into<HashMap<String, String>>>(hosts: T) -> Self {
    Self {
      uri_resolver: Box::new(MappingLocalUriResolver::new(hosts)),
      cache: DashMap::default(),
    }
  }
}

#[async_trait]
impl super::Protocol for LocalProtocol {
  async fn handle(
    &self,
    request: http::Request<Vec<u8>>,
  ) -> crate::Result<http::Response<Cow<'static, [u8]>>> {
    let url = self
      .uri_resolver
      .get_localhost_uri(request.uri())
      .ok_or(crate::Error::CannotResolveLocalHost)?;

    let mut builder = http::Response::builder();

    let client = reqwest::ClientBuilder::new();
    let mut proxy_builder = client.build()?.request(request.method().clone(), &url);
    proxy_builder = proxy_builder.body(request.body().clone());
    let r = proxy_builder.send().await?;
    let mut response = None;
    if r.status() == http::StatusCode::NOT_MODIFIED {
      response = self.cache.get(&url)
    }
    let response = if let Some(r) = response {
      r
    } else {
      let status = r.status();
      let headers = r.headers().clone();
      let body = r.bytes().await?;
      let response = CachedResponse {
        status,
        headers,
        body,
      };
      self.cache.insert(url.to_string(), response);
      self.cache.get(&url).unwrap()
    };
    for (name, value) in &response.headers {
      builder = builder.header(name, value);
    }
    let resp = builder
      .status(response.status)
      .body(response.body.to_vec().into())?;
    Ok(resp)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::protocol::Protocol;
  use http;
  use std::collections::HashMap;
  use std::iter;
  use std::net::{SocketAddr, TcpListener};
  use std::sync::Arc;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use tiny_http::{Header as TinyHeader, Method, Response as TinyResponse, Server as TinyServer};

  fn server() -> (SocketAddr, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = TinyServer::from_listener(listener, None).unwrap();

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_for_thread = counter.clone();

    let handle = std::thread::spawn(move || {
      for request in server.incoming_requests() {
        let n = counter_for_thread.fetch_add(1, Ordering::SeqCst) + 1;
        if request.method() == &Method::Get && request.url().starts_with("/index.html") {
          if n == 1 {
            let mut resp = TinyResponse::from_string("Hello World");
            resp.add_header(TinyHeader::from_bytes("Content-Type", "text/plain").unwrap());
            resp.add_header(TinyHeader::from_bytes("ETag", "\"v1\"").unwrap());
            let _ = request.respond(resp);
          } else {
            // After first response, server will return 304 because content is not changed.
            let mut resp = TinyResponse::empty(304);
            resp.add_header(TinyHeader::from_bytes("ETag", "\"v1\"").unwrap());
            let _ = request.respond(resp);
          }
        } else {
          let _ = request.respond(TinyResponse::empty(404));
        }
      }
    });

    (addr, handle)
  }

  #[tokio::test]
  async fn smoke() {
    let (addr, _) = server();
    let mapping =
      iter::once(("app.wvb".to_string(), format!("http://{addr}"))).collect::<HashMap<_, _>>();
    let protocol = LocalProtocol::new(mapping);

    let first_req = http::Request::builder()
      .uri("https://app.wvb/index.html")
      .method("GET")
      .body(Vec::new())
      .unwrap();
    let first_resp = protocol.handle(first_req).await.unwrap();
    assert_eq!(first_resp.status(), 200);
    assert_eq!(
      first_resp.headers().get("content-type").unwrap(),
      "text/plain"
    );
    assert_eq!(first_resp.body().as_ref(), b"Hello World");

    let second_req = http::Request::builder()
      .uri("https://app.wvb/index.html")
      .method("GET")
      .body(Vec::new())
      .unwrap();
    let second_resp = protocol.handle(second_req).await.unwrap();
    assert_eq!(second_resp.status(), 200);
    assert_eq!(
      second_resp.headers().get("content-type").unwrap(),
      "text/plain"
    );
    assert_eq!(second_resp.headers().get("etag").unwrap(), "\"v1\"");
    assert_eq!(first_resp.body().as_ref(), b"Hello World");
  }
}
