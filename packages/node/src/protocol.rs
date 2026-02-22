use crate::http::HttpMethod;
use crate::http::HttpResponse;
use crate::http::request;
use crate::source::BundleSource;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::collections::HashMap;
use std::sync::Arc;
use wvb::protocol;
use wvb::protocol::Protocol;

/// Protocol handler for serving files from bundle sources.
///
/// Serves web resources from `.wvb` bundle files, supporting:
/// - GET and HEAD HTTP methods
/// - HTTP Range requests for streaming
/// - Content-Type and custom HTTP headers
///
/// @example
/// ```typescript
/// const source = new BundleSource({
///   builtinDir: "./bundles/builtin",
///   remoteDir: "./bundles/remote"
/// });
///
/// const protocol = new BundleProtocol(source);
///
/// // Handle a request
/// const response = await protocol.handle("GET", "bundle://app/index.html");
/// console.log(`Status: ${response.status}`);
/// console.log(`Content-Type: ${response.headers["content-type"]}`);
/// ```
#[napi]
pub struct BundleProtocol {
  pub(crate) inner: Arc<protocol::BundleProtocol>,
}

#[napi]
impl BundleProtocol {
  /// Creates a new bundle protocol handler.
  ///
  /// @param {BundleSource} source - Bundle source to serve files from
  ///
  /// @example
  /// ```typescript
  /// const source = new BundleSource({
  ///   builtinDir: "./bundles",
  ///   remoteDir: "./remote"
  /// });
  /// const protocol = new BundleProtocol(source);
  /// ```
  #[napi(constructor)]
  pub fn new(source: &BundleSource) -> BundleProtocol {
    Self {
      inner: Arc::new(protocol::BundleProtocol::new(source.inner.clone())),
    }
  }

  /// Handles an HTTP request and returns a response.
  ///
  /// Processes requests in the format `scheme://bundle_name/path/to/file`.
  ///
  /// @param {HttpMethod} method - HTTP method (GET or HEAD)
  /// @param {string} uri - Request URI (e.g., "bundle://app/index.html")
  /// @param {Record<string, string>} [headers] - Optional request headers
  /// @returns {Promise<HttpResponse>} HTTP response
  ///
  /// @example
  /// ```typescript
  /// // GET request
  /// const response = await protocol.handle("GET", "bundle://app/index.html");
  /// if (response.status === 200) {
  ///   console.log(response.body.toString("utf-8"));
  /// }
  /// ```
  ///
  /// @example
  /// ```typescript
  /// // Range request for streaming
  /// const response = await protocol.handle(
  ///   "GET",
  ///   "bundle://app/video.mp4",
  ///   { "Range": "bytes=0-1023" }
  /// );
  /// console.log(`Status: ${response.status}`); // 206 Partial Content
  /// ```
  #[napi]
  pub fn handle(
    &self,
    env: Env,
    method: HttpMethod,
    uri: String,
    headers: Option<HashMap<String, String>>,
  ) -> crate::Result<AsyncBlock<HttpResponse>> {
    let req = request(method, uri, headers)?;
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

/// Protocol handler that proxies requests to localhost servers.
///
/// Forwards requests to local development servers for hot-reloading workflows.
/// Features response caching and 304 Not Modified support.
///
/// @example
/// ```typescript
/// const protocol = new LocalProtocol({
///   "myapp": "http://localhost:3000",
///   "api": "http://localhost:8080"
/// });
///
/// // This proxies to http://localhost:3000/index.html
/// const response = await protocol.handle("GET", "app://myapp/index.html");
/// ```
#[napi]
pub struct LocalProtocol {
  pub(crate) inner: Arc<protocol::LocalProtocol>,
}

#[napi]
impl LocalProtocol {
  /// Creates a new local protocol handler.
  ///
  /// @param {Record<string, string>} hosts - Map of custom hosts to localhost URLs
  ///
  /// @example
  /// ```typescript
  /// const protocol = new LocalProtocol({
  ///   "myapp": "http://localhost:3000",
  ///   "api": "http://localhost:8080"
  /// });
  /// ```
  #[napi(constructor)]
  pub fn new(hosts: HashMap<String, String>) -> LocalProtocol {
    Self {
      inner: Arc::new(protocol::LocalProtocol::new(hosts)),
    }
  }

  /// Handles an HTTP request by proxying to localhost.
  ///
  /// Maps custom protocol URIs to localhost URLs and forwards the request.
  ///
  /// @param {HttpMethod} method - HTTP method
  /// @param {string} uri - Request URI (e.g., "app://myapp/api/data")
  /// @param {Record<string, string>} [headers] - Optional request headers
  /// @returns {Promise<HttpResponse>} HTTP response from localhost
  ///
  /// @example
  /// ```typescript
  /// // Proxies to http://localhost:3000/api/data?foo=bar
  /// const response = await protocol.handle(
  ///   "GET",
  ///   "app://myapp/api/data?foo=bar"
  /// );
  /// console.log(response.status);
  /// ```
  ///
  /// @example
  /// ```typescript
  /// // POST with headers
  /// const response = await protocol.handle(
  ///   "POST",
  ///   "app://api/submit",
  ///   { "Content-Type": "application/json" }
  /// );
  /// ```
  #[napi]
  pub fn handle(
    &self,
    env: Env,
    method: HttpMethod,
    uri: String,
    headers: Option<HashMap<String, String>>,
  ) -> crate::Result<AsyncBlock<HttpResponse>> {
    let req = request(method, uri, headers)?;
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
