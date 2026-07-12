use async_trait::async_trait;
use http;
use http::Uri;
use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

pub type ProxyResolveFn = dyn Fn(
    &Uri,
  ) -> Pin<
    Box<
      dyn Future<Output = Result<Option<String>, Box<dyn std::error::Error + Send + Sync + 'static>>>
        + Send
        + 'static,
    >,
  > + Send
  + Sync;

#[non_exhaustive]
#[derive(Clone)]
pub enum ProxyResolver {
  HostMapping(HashMap<String, String>),
  Custom(Arc<ProxyResolveFn>),
}

impl ProxyResolver {
  /// Proxy to a target resolved from a static host mapping.
  /// e.g. `[("app.wvb", "http://localhost:3000")]` proxies `https://app.wvb/index.html`
  /// to `http://localhost:3000`. Hosts are matched as-is.
  pub fn host_mapping<I, K, V>(mapping: I) -> Self
  where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
  {
    Self::HostMapping(
      mapping
        .into_iter()
        .map(|(host, target)| (host.into(), target.into()))
        .collect(),
    )
  }

  /// Resolve the proxy target with a custom async closure. `Ok(None)` means "do not proxy".
  ///
  /// The future must be `'static`, so it cannot borrow `uri`; copy out what it needs
  /// (e.g. `let host = uri.host().map(str::to_owned);`) before the `async move` block.
  /// Inside the block, `?` boxes any `std::error::Error + Send + Sync` automatically.
  pub fn custom<F, Fut>(resolve_fn: F) -> Self
  where
    F: Fn(&Uri) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Option<String>, Box<dyn std::error::Error + Send + Sync + 'static>>>
      + Send
      + 'static,
  {
    Self::Custom(Arc::new(move |uri| Box::pin(resolve_fn(uri))))
  }

  /// Resolve the proxy target for the given uri, e.g. `http://localhost:3000`.
  /// [`ProxyProtocol`] appends the path and query of the original request to it.
  pub async fn resolve(&self, uri: &Uri) -> crate::Result<Option<String>> {
    match self {
      Self::HostMapping(mapping) => match uri.host() {
        Some(host) => Ok(mapping.get(host).map(|x| x.to_string()).to_owned()),
        None => Ok(None),
      },
      Self::Custom(resolver) => {
        let resolved = resolver(uri)
          .await
          .map_err(|_| crate::Error::CannotResolveProxyServer)?;
        Ok(resolved)
      }
    }
  }
}

fn proxy_url(target: &str, uri: &Uri) -> String {
  format!(
    "{}/{}{}",
    target.trim_end_matches('/'),
    uri.path().trim_start_matches('/'),
    match uri.query() {
      Some(query) => format!("?{query}"),
      None => String::new(),
    }
  )
}

#[derive(Clone)]
struct CachedResponse {
  status: http::StatusCode,
  headers: http::HeaderMap,
  body: bytes::Bytes,
}

/// Total body bytes [`ProxyProtocol`] keeps cached, unless
/// [`ProxyProtocol::with_max_cache_bytes`] says otherwise.
pub const DEFAULT_MAX_CACHE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Default)]
struct CacheState {
  entries: HashMap<String, CachedResponse>,
  /// Insertion order of `entries`, evicted from the front.
  order: VecDeque<String>,
  bytes: usize,
}

impl CacheState {
  fn remove(&mut self, url: &str) {
    if let Some(previous) = self.entries.remove(url) {
      self.bytes -= previous.body.len();
      if let Some(index) = self.order.iter().position(|x| x == url) {
        self.order.remove(index);
      }
    }
  }
}

/// Successful upstream responses, kept only so an upstream `304 Not Modified` can be answered with
/// the body we last saw for that url.
struct ResponseCache {
  state: Mutex<CacheState>,
  max_bytes: usize,
}

impl ResponseCache {
  fn new(max_bytes: usize) -> Self {
    Self {
      state: Mutex::new(CacheState::default()),
      max_bytes,
    }
  }

  /// A panic elsewhere in the process must not take the proxy down with a poisoned cache.
  fn lock(&self) -> MutexGuard<'_, CacheState> {
    self.state.lock().unwrap_or_else(|e| e.into_inner())
  }

  fn get(&self, url: &str) -> Option<CachedResponse> {
    self.lock().entries.get(url).cloned()
  }

  fn insert(&self, url: &str, response: CachedResponse) {
    let size = response.body.len();
    // A single response over the budget (or any response, with the cache off) is served straight
    // through, never cached.
    if self.max_bytes == 0 || size > self.max_bytes {
      self.lock().remove(url);
      return;
    }
    let mut state = self.lock();
    state.remove(url);
    state.bytes += size;
    state.entries.insert(url.to_string(), response);
    state.order.push_back(url.to_string());
    while state.bytes > self.max_bytes {
      let Some(oldest) = state.order.pop_front() else {
        break;
      };
      if let Some(evicted) = state.entries.remove(&oldest) {
        state.bytes -= evicted.body.len();
      }
    }
  }
}

/// Protocol handler that proxies requests to servers.
///
/// `ProxyProtocol` forwards requests to local development servers, making it
/// easy to develop webview applications with hot-reloading.
///
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "protocol-proxy")]
/// # async {
/// use wvb::protocol::{Protocol, ProxyProtocol, ProxyResolver};
///
/// let protocol = ProxyProtocol::new(
///   ProxyResolver::host_mapping([
///     ("myapp", "http://localhost:3000")
///   ])
/// );
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
pub struct ProxyProtocol {
  resolver: ProxyResolver,
  /// Built on the first request and reused, so proxied requests share a connection pool.
  client: OnceLock<reqwest::Client>,
  cache: ResponseCache,
}

impl ProxyProtocol {
  /// Creates a new `ProxyProtocol`.
  ///
  /// # Arguments
  ///
  /// * `resolver` - Resolves the proxy target for a request uri (a static host mapping, or a
  ///   custom resolver).
  ///
  /// # Example
  ///
  /// ```
  /// # #[cfg(feature = "protocol-proxy")]
  /// # {
  /// use wvb::protocol::{ProxyProtocol, ProxyResolver};
  ///
  /// let protocol = ProxyProtocol::new(
  ///   ProxyResolver::host_mapping([
  ///     ("myapp", "http://localhost:3000"),
  ///     ("otherapp", "http://localhost:3001"),
  ///   ])
  /// );
  /// # }
  /// ```
  pub fn new(resolver: ProxyResolver) -> Self {
    Self {
      resolver,
      client: OnceLock::new(),
      cache: ResponseCache::new(DEFAULT_MAX_CACHE_BYTES),
    }
  }

  /// How many bytes of upstream response bodies to keep cached (default:
  /// [`DEFAULT_MAX_CACHE_BYTES`], 32 MiB). `0` disables the cache, and an upstream `304` is then
  /// passed through as-is.
  ///
  /// ```
  /// # #[cfg(feature = "protocol-proxy")]
  /// # {
  /// use wvb::protocol::{ProxyProtocol, ProxyResolver};
  ///
  /// let protocol = ProxyProtocol::new(ProxyResolver::host_mapping([
  ///   ("myapp", "http://localhost:3000"),
  /// ]))
  /// .with_max_cache_bytes(8 * 1024 * 1024);
  /// # }
  /// ```
  pub fn with_max_cache_bytes(mut self, max_cache_bytes: usize) -> Self {
    self.cache = ResponseCache::new(max_cache_bytes);
    self
  }

  fn client(&self) -> crate::Result<&reqwest::Client> {
    if let Some(client) = self.client.get() {
      return Ok(client);
    }
    let client = reqwest::ClientBuilder::new().build()?;
    Ok(self.client.get_or_init(|| client))
  }
}

#[async_trait]
impl super::Protocol for ProxyProtocol {
  #[cfg_attr(feature = "tracing", tracing::instrument(
    skip_all,
    fields(request.method = request.method().to_string(), request.uri = request.uri().to_string()),
    err(level = "error")
  ))]
  async fn handle(
    &self,
    request: http::Request<Vec<u8>>,
  ) -> crate::Result<http::Response<Cow<'static, [u8]>>> {
    let target = self
      .resolver
      .resolve(request.uri())
      .await?
      .ok_or(crate::Error::CannotResolveProxyServer)?;
    let url = proxy_url(&target, request.uri());

    #[cfg(feature = "tracing")]
    tracing::info!(localhost_uri = url);

    let mut builder = http::Response::builder();

    let mut proxy_builder = self
      .client()?
      .request(request.method().clone(), &url)
      .headers(request.headers().clone());
    proxy_builder = proxy_builder.body(request.body().clone());
    let r = proxy_builder.send().await?;

    // The webview only gets `304` back if it already holds the resource; when the upstream answers
    // one for a body we cached, serve that body instead of an empty response.
    let cached = (r.status() == http::StatusCode::NOT_MODIFIED)
      .then(|| self.cache.get(&url))
      .flatten();
    let response = match cached {
      Some(response) => response,
      None => {
        let status = r.status();
        let headers = r.headers().clone();
        let body = r.bytes().await?;
        let response = CachedResponse {
          status,
          headers,
          body,
        };
        // Only a whole `200` body can stand in for a later `304`; a `206` is a slice of one, and
        // an error status is not worth replaying.
        if status == http::StatusCode::OK {
          self.cache.insert(&url, response.clone());
        }
        response
      }
    };
    for (name, value) in &response.headers {
      builder = builder.header(name, value);
    }
    let resp = builder
      .status(response.status)
      .body(response.body.to_vec().into())?;
    #[cfg(feature = "tracing")]
    {
      use crate::protocol::http_ext::HttpHeadersTracingInfo;
      tracing::info!(
        response.status = resp.status().as_u16(),
        response.headers = resp.headers().tracing_info()
      );
    }
    Ok(resp)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::protocol::Protocol;
  use http;
  use std::collections::HashMap;
  use std::net::{SocketAddr, TcpListener};
  use std::sync::Arc;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use tiny_http::{Header as TinyHeader, Method, Response as TinyResponse, Server as TinyServer};

  fn uri(s: &str) -> Uri {
    s.parse().unwrap()
  }

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
  async fn proxy_host_mapping() {
    let r = ProxyResolver::host_mapping([("app.wvb", "http://localhost:3000")]);
    assert_eq!(
      r.resolve(&uri("https://app.wvb/index.html"))
        .await
        .unwrap()
        .as_deref(),
      Some("http://localhost:3000")
    );
    assert_eq!(r.resolve(&uri("https://other.wvb/")).await.unwrap(), None);

    // Also accepts an owned map.
    let mapping = HashMap::from([("app.wvb".to_owned(), "http://localhost:3000".to_owned())]);
    let r = ProxyResolver::host_mapping(mapping);
    assert_eq!(
      r.resolve(&uri("https://app.wvb/"))
        .await
        .unwrap()
        .as_deref(),
      Some("http://localhost:3000")
    );
  }

  #[tokio::test]
  async fn proxy_custom() {
    let r = ProxyResolver::custom(|uri| {
      let host = uri.host().map(str::to_owned);
      async move { Ok(host.map(|host| format!("http://{host}:3000"))) }
    });
    assert_eq!(
      r.resolve(&uri("https://app.wvb/index.html"))
        .await
        .unwrap()
        .as_deref(),
      Some("http://app.wvb:3000")
    );

    // Any `std::error::Error + Send + Sync` boxes into the resolver error via `?`.
    let r = ProxyResolver::custom(|_| async move {
      let port = "not-a-port".parse::<u16>()?;
      Ok(Some(format!("http://localhost:{port}")))
    });
    let err = r.resolve(&uri("https://app.wvb/")).await.unwrap_err();
    assert!(matches!(err, crate::Error::CannotResolveProxyServer));
  }

  #[test]
  fn proxy_url_joins_path_and_query() {
    assert_eq!(
      proxy_url(
        "http://localhost:3000",
        &uri("app://myapp/api/data?foo=bar")
      ),
      "http://localhost:3000/api/data?foo=bar"
    );
    // Trailing slash of the target is not duplicated.
    assert_eq!(
      proxy_url("http://localhost:3000/", &uri("app://myapp/")),
      "http://localhost:3000/"
    );
  }

  #[test]
  fn proxy_url_keeps_the_path_encoded() {
    // Decoding these would forward `/api?debug=true` and `/a/b` — other resources than the ones
    // the webview requested.
    assert_eq!(
      proxy_url(
        "http://localhost:3000",
        &uri("app://myapp/api%3Fdebug=true")
      ),
      "http://localhost:3000/api%3Fdebug=true"
    );
    assert_eq!(
      proxy_url("http://localhost:3000", &uri("app://myapp/a%2Fb/c%20d.js")),
      "http://localhost:3000/a%2Fb/c%20d.js"
    );
  }

  fn cached(size: usize) -> CachedResponse {
    CachedResponse {
      status: http::StatusCode::OK,
      headers: http::HeaderMap::new(),
      body: bytes::Bytes::from(vec![0u8; size]),
    }
  }

  #[test]
  fn response_cache_stays_within_its_budget() {
    let cache = ResponseCache::new(10);
    cache.insert("/a", cached(6));
    cache.insert("/b", cached(6));
    // 12 bytes over a 10-byte budget: the oldest entry goes.
    assert!(cache.get("/a").is_none());
    assert!(cache.get("/b").is_some());
    assert_eq!(cache.lock().bytes, 6);

    // Re-caching a url replaces its entry rather than counting it twice.
    cache.insert("/b", cached(8));
    assert_eq!(cache.lock().entries.len(), 1);
    assert_eq!(cache.lock().bytes, 8);

    // A response larger than the whole budget is never cached.
    cache.insert("/big", cached(11));
    assert!(cache.get("/big").is_none());
    assert_eq!(cache.lock().bytes, 8);

    // A zero budget caches nothing at all.
    let off = ResponseCache::new(0);
    off.insert("/a", cached(0));
    assert!(off.get("/a").is_none());
  }

  #[tokio::test]
  async fn with_max_cache_bytes_off_passes_the_upstream_304_through() {
    let (addr, _) = server();
    let protocol = ProxyProtocol::new(ProxyResolver::host_mapping([(
      "app.wvb",
      format!("http://{addr}"),
    )]))
    .with_max_cache_bytes(0);

    let request = || {
      http::Request::builder()
        .uri("scheme://app.wvb/index.html")
        .method("GET")
        .body(Vec::new())
        .unwrap()
    };
    assert_eq!(protocol.handle(request()).await.unwrap().status(), 200);
    // The test server answers 304 from the second request on. With no cached body to serve in its
    // place, the webview gets the 304 itself.
    let second = protocol.handle(request()).await.unwrap();
    assert_eq!(second.status(), 304);
    assert!(second.body().is_empty());
  }

  #[tokio::test]
  async fn smoke() {
    let (addr, _) = server();
    let protocol = ProxyProtocol::new(ProxyResolver::host_mapping([(
      "app.wvb",
      format!("http://{addr}"),
    )]));

    let first_req = http::Request::builder()
      .uri("scheme://app.wvb/index.html")
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
      .uri("scheme://app.wvb/index.html")
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
