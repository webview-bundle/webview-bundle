use crate::http::{HttpMethod, HttpResponse, request};
use crate::source::BundleSource;
use std::collections::HashMap;
use std::sync::Arc;
use wvb::protocol;
use wvb::protocol::Protocol;

/// Which hostname segment is used as the bundle name.
#[derive(uniffi::Enum, Clone, Debug)]
pub enum HostnameSegment {
  /// First segment. (e.g. `app.mydomain.com` -> `app`)
  First,
  /// Full hostname. (e.g. `app.wvb` -> `app.wvb`)
  Full,
  /// Strip the last segment. (e.g. `a.b.wvb` -> `a.b`)
  StripSuffix,
  /// The nth segment, 0-based.
  Nth { index: u32 },
}

impl From<HostnameSegment> for protocol::HostnameSegment {
  fn from(v: HostnameSegment) -> Self {
    match v {
      HostnameSegment::First => Self::First,
      HostnameSegment::Full => Self::Full,
      HostnameSegment::StripSuffix => Self::StripSuffix,
      HostnameSegment::Nth { index } => Self::Nth(index as usize),
    }
  }
}

/// How the bundle name is resolved from the request uri.
#[derive(uniffi::Enum, Clone, Debug)]
pub enum BundleResolver {
  /// From the uri hostname.
  ///
  /// - `segment`: which part of the host to use (default: [`HostnameSegment::First`]).
  /// - `allow_wvb_suffix_only`: only resolve hosts ending in `.wvb` (default: false).
  Hostname {
    segment: Option<HostnameSegment>,
    allow_wvb_suffix_only: Option<bool>,
  },
  /// From the uri pathname.
  ///
  /// - `segment_index`: 0-based over non-empty path segments (default: 0).
  ///   e.g. `app://_/my-app/index.html` with index 0 -> bundle `my-app`.
  Pathname { segment_index: Option<u32> },
}

impl From<BundleResolver> for protocol::UriBundleResolver {
  fn from(v: BundleResolver) -> Self {
    match v {
      BundleResolver::Hostname {
        segment,
        allow_wvb_suffix_only,
      } => Self::hostname(segment.map(Into::into), allow_wvb_suffix_only),
      BundleResolver::Pathname { segment_index } => {
        Self::pathname(segment_index.map(|x| x as usize))
      }
    }
  }
}

/// How the file path in the bundle is resolved from the request uri.
#[derive(uniffi::Enum, Clone, Debug)]
pub enum PathResolver {
  /// Use the uri path as-is (only percent-decoded).
  Exact,
  /// Directory index: `/` -> `/index.html` and `/about` -> `/about/index.html`.
  DirectoryIndex,
  /// `.html` extension: `/` -> `/index.html` and `/about` -> `/about.html`.
  HtmlExtension,
}

impl From<PathResolver> for protocol::UriPathResolver {
  fn from(v: PathResolver) -> Self {
    match v {
      PathResolver::Exact => Self::exact(),
      PathResolver::DirectoryIndex => Self::directory_index(),
      PathResolver::HtmlExtension => Self::html_extension(),
    }
  }
}

/// How a [`BundleProtocolHandler`] resolves the request uri.
///
/// Defaults to the first hostname segment as the bundle name, and a directory-index path
/// (`/about` -> `/about/index.html`).
#[derive(uniffi::Record, Clone, Debug, Default)]
pub struct BundleProtocolOptions {
  #[uniffi(default = None)]
  pub bundle_resolver: Option<BundleResolver>,
  #[uniffi(default = None)]
  pub path_resolver: Option<PathResolver>,
}

/// Handles HTTP-like requests by serving bundle entries from a [`BundleSource`].
///
/// By default the host portion of the URI identifies the bundle by name
/// (e.g. `https://app.wvb/index.html` → bundle `"app"`, path `"/index.html"`);
/// pass [`BundleProtocolOptions`] to resolve the bundle name or the path differently.
/// Returns 200 with the entry body, 404 when the path is not found, or
/// 200 with an empty body for HEAD requests.
#[derive(uniffi::Object)]
pub struct BundleProtocolHandler {
  inner: Arc<protocol::BundleProtocol>,
}

#[uniffi::export]
impl BundleProtocolHandler {
  #[uniffi::constructor(default(options = None))]
  pub fn new(
    source: Arc<BundleSource>,
    options: Option<BundleProtocolOptions>,
  ) -> Arc<BundleProtocolHandler> {
    let mut inner = protocol::BundleProtocol::new(source.inner.clone());
    if let Some(options) = options {
      if let Some(bundle_resolver) = options.bundle_resolver {
        inner = inner.with_bundle_resolver(bundle_resolver.into());
      }
      if let Some(path_resolver) = options.path_resolver {
        inner = inner.with_path_resolver(path_resolver.into());
      }
    }
    Arc::new(BundleProtocolHandler {
      inner: Arc::new(inner),
    })
  }
}

#[uniffi::export(async_runtime = "tokio")]
impl BundleProtocolHandler {
  /// Serves the request from the bundle. `body` is accepted for a uniform call shape across
  /// handlers, but unused: only GET and HEAD are served.
  #[uniffi::method(default(headers = None, body = None))]
  pub async fn handle(
    &self,
    method: HttpMethod,
    uri: String,
    headers: Option<HashMap<String, String>>,
    body: Option<Vec<u8>>,
  ) -> Result<HttpResponse, crate::Error> {
    let req = request(method, uri, headers, body)?;
    let resp = self.inner.handle(req).await?;
    Ok(HttpResponse::from(resp))
  }
}

/// Resolves the proxy target for a request uri, e.g. `http://localhost:3000`.
///
/// The path and query of the request are appended to the returned target. Return `null` to
/// not proxy the request; the handler then fails with `CannotResolveProxyServer`.
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait ProxyResolver: Send + Sync {
  async fn resolve(&self, uri: String) -> Option<String>;
}

/// How a [`ProxyProtocolHandler`] behaves beyond resolving the target.
///
/// - `max_cache_bytes`: how many bytes of upstream response bodies to keep, so an upstream
///   `304 Not Modified` can be answered with the body last seen for that url (default: 32 MiB;
///   `0` turns the cache off and passes the `304` through).
#[derive(uniffi::Record, Clone, Debug, Default)]
pub struct ProxyProtocolOptions {
  #[uniffi(default = None)]
  pub max_cache_bytes: Option<u32>,
}

impl ProxyProtocolOptions {
  fn apply(self, protocol: protocol::ProxyProtocol) -> protocol::ProxyProtocol {
    match self.max_cache_bytes {
      Some(max_cache_bytes) => protocol.with_max_cache_bytes(max_cache_bytes as usize),
      None => protocol,
    }
  }
}

/// Proxies HTTP-like requests to another HTTP server (typically a local dev server).
///
/// The target is resolved per request — either from a static host mapping
/// (e.g. `{"myapp": "http://localhost:8080"}`, keyed by uri host) or by a custom
/// [`ProxyResolver`]. A request whose target cannot be resolved fails with an error.
#[derive(uniffi::Object)]
pub struct ProxyProtocolHandler {
  inner: Arc<protocol::ProxyProtocol>,
}

#[uniffi::export]
impl ProxyProtocolHandler {
  /// Proxy by a static host mapping, keyed by the uri host.
  #[uniffi::constructor(default(options = None))]
  pub fn new(
    hosts: HashMap<String, String>,
    options: Option<ProxyProtocolOptions>,
  ) -> Arc<ProxyProtocolHandler> {
    let resolver = protocol::ProxyResolver::host_mapping(hosts);
    Arc::new(ProxyProtocolHandler::build(resolver, options))
  }

  /// Proxy by a custom resolver.
  #[uniffi::constructor(name = "custom", default(options = None))]
  pub fn custom(
    resolver: Arc<dyn ProxyResolver>,
    options: Option<ProxyProtocolOptions>,
  ) -> Arc<ProxyProtocolHandler> {
    let resolver = protocol::ProxyResolver::custom(move |uri| {
      let uri = uri.to_string();
      let resolver = resolver.clone();
      async move { Ok(resolver.resolve(uri).await) }
    });
    Arc::new(ProxyProtocolHandler::build(resolver, options))
  }
}

impl ProxyProtocolHandler {
  fn build(
    resolver: protocol::ProxyResolver,
    options: Option<ProxyProtocolOptions>,
  ) -> ProxyProtocolHandler {
    let mut inner = protocol::ProxyProtocol::new(resolver);
    if let Some(options) = options {
      inner = options.apply(inner);
    }
    ProxyProtocolHandler {
      inner: Arc::new(inner),
    }
  }
}

#[uniffi::export(async_runtime = "tokio")]
impl ProxyProtocolHandler {
  /// Forwards the request — including `body`, for POST/PUT/PATCH — to the resolved target.
  #[uniffi::method(default(headers = None, body = None))]
  pub async fn handle(
    &self,
    method: HttpMethod,
    uri: String,
    headers: Option<HashMap<String, String>>,
    body: Option<Vec<u8>>,
  ) -> Result<HttpResponse, crate::Error> {
    let req = request(method, uri, headers, body)?;
    let resp = self.inner.handle(req).await?;
    Ok(HttpResponse::from(resp))
  }
}
