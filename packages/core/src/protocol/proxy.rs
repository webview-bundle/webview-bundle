use async_trait::async_trait;
use dashmap::DashMap;
use http;
use http::Uri;
use std::borrow::Cow;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

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

/// Join the resolved proxy target with the path and query of the original request.
/// e.g. target `http://localhost:3000` + `app://myapp/api/data?foo=bar`
/// -> `http://localhost:3000/api/data?foo=bar`
fn proxy_url(target: &str, uri: &Uri) -> String {
  let path = percent_encoding::percent_decode(uri.path().as_bytes()).decode_utf8_lossy();
  format!(
    "{}/{}{}",
    target.trim_end_matches('/'),
    path.trim_start_matches('/'),
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
/// use wvb::protocol::{ProxyProtocol, ProxyResolver};
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
  cache: DashMap<String, CachedResponse>,
}

impl ProxyProtocol {
  /// Creates a new `ProxyProtocol`.
  ///
  /// # Arguments
  ///
  /// * `resolver` - Resolver that matches to local server when the
  /// request uri given.
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
      cache: DashMap::default(),
    }
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

    let client = reqwest::ClientBuilder::new();
    let mut proxy_builder = client.build()?.request(request.method().clone(), &url);
    proxy_builder = proxy_builder.headers(request.headers().clone());
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
