use crate::http::HttpMethod;
use crate::http::HttpResponse;
use crate::http::request;
use crate::js::{JsCallback, JsCallbackExt};
use crate::source::Source;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::collections::HashMap;
use std::sync::Arc;
use wvb::protocol;
use wvb::protocol::Protocol;

/// Which hostname segment is used as the bundle name.
///
/// A number can be given instead to pick the nth segment (0-based).
///
/// @enum {string}
#[napi(string_enum = "snake_case")]
pub enum HostnameSegment {
  /// First segment. (e.g. `app.mydomain.com` -> `app`)
  First,
  /// Full hostname. (e.g. `app.wvb` -> `app.wvb`)
  Full,
  /// Strip the last segment. (e.g. `a.b.wvb` -> `a.b`)
  StripSuffix,
}

/// Options for resolving the bundle name from the request uri.
///
/// - `hostname`: from the uri hostname.
///   - `segment` - Hostname segment to use, or the nth segment (default: 'first')
///   - `allowWvbSuffixOnly` - Only resolve hosts ending in `.wvb` (default: false)
/// - `pathname`: from the uri pathname.
///   - `segmentIndex` - Path segment index, 0-based over non-empty segments (default: 0)
///
/// @example
/// ```typescript
/// // `https://app.wvb/index.html` -> bundle "app"
/// const byHostname: UriBundleResolver = { type: 'hostname' };
///
/// // `https://cdn.example.com/my-app/index.html` -> bundle "my-app"
/// const byPathname: UriBundleResolver = { type: 'pathname', segmentIndex: 0 };
/// ```
#[napi(discriminant_case = "snake_case", object_to_js = false)]
pub enum UriBundleResolver {
  Hostname {
    segment: Option<Either<HostnameSegment, u32>>,
    allow_wvb_suffix_only: Option<bool>,
  },
  Pathname {
    segment_index: Option<u32>,
  },
}

impl From<UriBundleResolver> for protocol::UriBundleResolver {
  fn from(value: UriBundleResolver) -> Self {
    match value {
      UriBundleResolver::Hostname {
        segment,
        allow_wvb_suffix_only,
      } => {
        let segment = segment.map(|segment| match segment {
          Either::A(HostnameSegment::First) => protocol::HostnameSegment::First,
          Either::A(HostnameSegment::Full) => protocol::HostnameSegment::Full,
          Either::A(HostnameSegment::StripSuffix) => protocol::HostnameSegment::StripSuffix,
          Either::B(index) => protocol::HostnameSegment::Nth(index as usize),
        });
        Self::hostname(segment, allow_wvb_suffix_only)
      }
      UriBundleResolver::Pathname { segment_index } => {
        Self::pathname(segment_index.map(|x| x as usize))
      }
    }
  }
}

/// How the file path in the bundle is resolved from the request uri.
///
/// @enum {string}
#[napi(string_enum = "snake_case")]
pub enum UriPathResolver {
  /// Use the uri path as-is (only percent-decoded).
  Exact,
  /// Directory index: `/` -> `/index.html` and `/about` -> `/about/index.html`.
  /// (static-site / MPA style; e.g. Astro `format: 'directory'` / Next `trailingSlash: true`)
  DirectoryIndex,
  /// `.html` extension: `/` -> `/index.html` and `/about` -> `/about.html`.
  /// (flat-file style; e.g. Astro `format: 'file'` / GitHub Pages / Next `trailingSlash: false`)
  HtmlExtension,
}

impl From<UriPathResolver> for protocol::UriPathResolver {
  fn from(value: UriPathResolver) -> Self {
    match value {
      UriPathResolver::Exact => Self::exact(),
      UriPathResolver::DirectoryIndex => Self::directory_index(),
      UriPathResolver::HtmlExtension => Self::html_extension(),
    }
  }
}

/// Options for the bundle protocol.
///
/// @property {UriBundleResolver} [bundleResolver] - How the bundle name is resolved (default: first hostname segment)
/// @property {UriPathResolver} [pathResolver] - How the file path is resolved (default: 'directory_index')
#[napi(object, object_to_js = false)]
pub struct BundleProtocolOptions {
  pub bundle_resolver: Option<UriBundleResolver>,
  pub path_resolver: Option<UriPathResolver>,
}

/// Protocol handler for serving files from bundle sources.
///
/// Serves web resources from `.wvb` bundle files, supporting:
/// - GET and HEAD HTTP methods
/// - HTTP Range requests for streaming
/// - Content-Type and custom HTTP headers
///
/// @example
/// ```typescript
/// const source = new Source({
///   builtinDir: './bundles/builtin',
///   remoteDir: './bundles/remote',
/// });
///
/// const protocol = new BundleProtocol(source);
///
/// // Handle a request
/// const response = await protocol.handle('get', 'bundle://app/index.html');
/// console.log(`Status: ${response.status}`);
/// console.log(`Content-Type: ${response.headers['content-type']}`);
/// ```
#[napi]
pub struct BundleProtocol {
  pub(crate) inner: Arc<protocol::BundleProtocol>,
}

#[napi]
impl BundleProtocol {
  /// Creates a new bundle protocol handler.
  ///
  /// @param {Source} source - Bundle source to serve files from
  /// @param {BundleProtocolOptions} [options] - How the request uri is resolved
  ///
  /// @example
  /// ```typescript
  /// const source = new Source({
  ///   builtinDir: './bundles',
  ///   remoteDir: './remote',
  /// });
  /// const protocol = new BundleProtocol(source);
  /// ```
  ///
  /// @example
  /// ```typescript
  /// // `https://cdn.example.com/my-app/about` -> "/about/index.html" of the "my-app" bundle
  /// const protocol = new BundleProtocol(source, {
  ///   bundleResolver: { type: 'pathname' },
  /// });
  /// ```
  ///
  /// @example
  /// ```typescript
  /// // Entry checksum verification is inherited from the source's read options; to serve
  /// // without checking entry checksums, configure it on the source instead.
  /// const source = new Source({
  ///   builtinDir: './bundles',
  ///   remoteDir: './remote',
  ///   options: { dataRead: { checksum: { verify: false } } },
  /// });
  /// const protocol = new BundleProtocol(source);
  /// ```
  #[napi(constructor)]
  pub fn new(source: &Source, options: Option<BundleProtocolOptions>) -> BundleProtocol {
    let mut inner = protocol::BundleProtocol::new(source.inner.clone());
    if let Some(options) = options {
      if let Some(bundle_resolver) = options.bundle_resolver {
        inner = inner.set_bundle_resolver(bundle_resolver.into());
      }
      if let Some(path_resolver) = options.path_resolver {
        inner = inner.set_path_resolver(path_resolver.into());
      }
    }
    Self {
      inner: Arc::new(inner),
    }
  }

  /// Handles an HTTP request and returns a response.
  ///
  /// Processes requests in the format `scheme://bundle_name/path/to/file`.
  ///
  /// @param {HttpMethod} method - HTTP method (GET or HEAD)
  /// @param {string} uri - Request URI (e.g., "bundle://app/index.html")
  /// @param {Record<string, string>} [headers] - Optional request headers
  /// @param {Buffer} [body] - Optional request body (accepted, but unused: only GET/HEAD are served)
  /// @returns {Promise<HttpResponse>} HTTP response
  ///
  /// @example
  /// ```typescript
  /// // GET request
  /// const response = await protocol.handle('get', 'bundle://app/index.html');
  /// if (response.status === 200) {
  ///   console.log(response.body.toString('utf-8'));
  /// }
  /// ```
  ///
  /// @example
  /// ```typescript
  /// // Range request for streaming
  /// const response = await protocol.handle('get', 'bundle://app/video.mp4', { Range: 'bytes=0-1023' });
  /// console.log(`Status: ${response.status}`); // 206 Partial Content
  /// ```
  #[napi]
  pub fn handle(
    &self,
    env: Env,
    method: HttpMethod,
    uri: String,
    headers: Option<HashMap<String, String>>,
    body: Option<Buffer>,
  ) -> crate::Result<AsyncBlock<HttpResponse>> {
    let req = request(method, uri, headers, body)?;
    let inner = self.inner.clone();
    let resp = AsyncBlockBuilder::new(async move {
      inner
        .handle(req)
        .await
        .map(HttpResponse::from)
        .map_err(crate::Error::Core)
        .map_err(|e| e.into())
    })
    .build(&env)?;
    Ok(resp)
  }
}

/// Resolves the proxy target for a request uri.
///
/// Either a static host mapping, or a function returning the target for a uri
/// (`null` to not proxy).
pub(crate) type ProxyResolver =
  Either<HashMap<String, String>, JsCallback<String, Promise<Option<String>>>>;

fn proxy_resolver(resolver: ProxyResolver) -> protocol::ProxyResolver {
  match resolver {
    Either::A(hosts) => protocol::ProxyResolver::host_mapping(hosts),
    Either::B(callback) => protocol::ProxyResolver::custom(move |uri| {
      let uri = uri.to_string();
      let callback = Arc::clone(&callback);
      async move {
        let resolved = callback.invoke_async(uri).await?.await?;
        Ok(resolved)
      }
    }),
  }
}

/// Protocol handler that proxies requests to other servers.
///
/// Forwards requests to local development servers for hot-reloading workflows.
///
/// @example
/// ```typescript
/// const protocol = new ProxyProtocol({
///   myapp: 'http://localhost:3000',
///   api: 'http://localhost:8080',
/// });
///
/// // This proxies to http://localhost:3000/index.html
/// const response = await protocol.handle('get', 'app://myapp/index.html');
/// ```
#[napi]
pub struct ProxyProtocol {
  pub(crate) inner: Arc<protocol::ProxyProtocol>,
}

#[napi]
impl ProxyProtocol {
  /// Creates a new proxy protocol handler.
  ///
  /// The resolver returns the target server for a request uri; the path and query of
  /// the request are appended to it. A static host mapping resolves by uri hostname,
  /// while a function receives the full uri and returns the target, or `null` to not
  /// proxy.
  ///
  /// @param {Record<string, string> | ((uri: string) => Promise<string | null>)} resolver - Host mapping or custom resolver
  ///
  /// @example
  /// ```typescript
  /// const protocol = new ProxyProtocol({
  ///   myapp: 'http://localhost:3000',
  ///   api: 'http://localhost:8080',
  /// });
  /// ```
  ///
  /// @example
  /// ```typescript
  /// // Proxies `app://myapp/index.html` to `http://localhost:3000/index.html`
  /// const protocol = new ProxyProtocol(async uri => {
  ///   const port = await lookupDevServer(new URL(uri).hostname);
  ///   return port != null ? `http://localhost:${port}` : null;
  /// });
  /// ```
  #[napi(
    constructor,
    ts_args_type = "resolver: Record<string, string> | ((uri: string) => Promise<string | null>)"
  )]
  pub fn new(resolver: ProxyResolver) -> ProxyProtocol {
    Self {
      inner: Arc::new(protocol::ProxyProtocol::new(proxy_resolver(resolver))),
    }
  }

  /// Handles an HTTP request by proxying it to the resolved server.
  ///
  /// @param {HttpMethod} method - HTTP method
  /// @param {string} uri - Request URI (e.g., "app://myapp/api/data")
  /// @param {Record<string, string>} [headers] - Optional request headers
  /// @param {Buffer} [body] - Optional request body, forwarded as-is (POST/PUT/PATCH)
  /// @returns {Promise<HttpResponse>} HTTP response from the proxied server
  ///
  /// @example
  /// ```typescript
  /// // Proxies to http://localhost:3000/api/data?foo=bar
  /// const response = await protocol.handle('get', 'app://myapp/api/data?foo=bar');
  /// console.log(response.status);
  /// ```
  ///
  /// @example
  /// ```typescript
  /// // POST with a body
  /// const response = await protocol.handle(
  ///   'post',
  ///   'app://api/submit',
  ///   { 'Content-Type': 'application/json' },
  ///   Buffer.from(JSON.stringify({ hello: 'world' })),
  /// );
  /// ```
  #[napi]
  pub fn handle(
    &self,
    env: Env,
    method: HttpMethod,
    uri: String,
    headers: Option<HashMap<String, String>>,
    body: Option<Buffer>,
  ) -> crate::Result<AsyncBlock<HttpResponse>> {
    let req = request(method, uri, headers, body)?;
    let inner = self.inner.clone();
    let resp = AsyncBlockBuilder::new(async move {
      inner
        .handle(req)
        .await
        .map(HttpResponse::from)
        .map_err(crate::Error::Core)
        .map_err(|e| e.into())
    })
    .build(&env)?;
    Ok(resp)
  }
}
