use async_trait::async_trait;
use http;
use http::Uri;
use std::borrow::Cow;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

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
  /// The future is `'static`, so copy what it needs out of `uri` before the `async move` block.
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
    }
  }

  fn client(&self) -> crate::Result<&reqwest::Client> {
    if let Some(client) = self.client.get() {
      return Ok(client);
    }
    // See `remote::HttpOptions::apply`: this feature selects the Ring provider explicitly
    // instead of compiling AWS-LC for every cross target.
    let _ = rustls::crypto::ring::default_provider().install_default();
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
    let response = proxy_builder.send().await?;

    let status = response.status();
    for (name, value) in response.headers() {
      builder = builder.header(name, value);
    }
    let body = response.bytes().await?;
    let resp = builder.status(status).body(body.to_vec().into())?;
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
  use tiny_http::{Header as TinyHeader, Method, Response as TinyResponse, Server as TinyServer};

  fn uri(s: &str) -> Uri {
    s.parse().unwrap()
  }

  /// Serves `/index.html` with an `ETag`, answering `304` to a request that already holds it.
  fn server() -> (SocketAddr, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = TinyServer::from_listener(listener, None).unwrap();

    let handle = std::thread::spawn(move || {
      for request in server.incoming_requests() {
        let known = request
          .headers()
          .iter()
          .any(|header| header.field.equiv("If-None-Match") && header.value == "\"v1\"");
        if request.method() == &Method::Get && request.url().starts_with("/index.html") {
          if known {
            let mut resp = TinyResponse::empty(304);
            resp.add_header(TinyHeader::from_bytes("ETag", "\"v1\"").unwrap());
            let _ = request.respond(resp);
          } else {
            let mut resp = TinyResponse::from_string("Hello World");
            resp.add_header(TinyHeader::from_bytes("Content-Type", "text/plain").unwrap());
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

  #[tokio::test]
  async fn smoke() {
    let (addr, _) = server();
    let protocol = ProxyProtocol::new(ProxyResolver::host_mapping([(
      "app.wvb",
      format!("http://{addr}"),
    )]));

    let request = http::Request::builder()
      .uri("scheme://app.wvb/index.html")
      .method("GET")
      .body(Vec::new())
      .unwrap();
    let response = protocol.handle(request).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
      response.headers().get("content-type").unwrap(),
      "text/plain"
    );
    assert_eq!(response.headers().get("etag").unwrap(), "\"v1\"");
    assert_eq!(response.body().as_ref(), b"Hello World");

    // A conditional request is forwarded, and so is the `304` the upstream answers it with.
    let conditional = http::Request::builder()
      .uri("scheme://app.wvb/index.html")
      .method("GET")
      .header("If-None-Match", "\"v1\"")
      .body(Vec::new())
      .unwrap();
    let response = protocol.handle(conditional).await.unwrap();
    assert_eq!(response.status(), 304);
    assert!(response.body().is_empty());
  }
}
