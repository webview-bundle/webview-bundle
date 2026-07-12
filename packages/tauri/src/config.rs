use std::path::PathBuf;
use std::sync::Arc;
use tauri::http;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, Runtime};
use wvb::remote;
use wvb::updater::UpdaterConfig;

pub use wvb::integrity::IntegrityPolicy;
pub use wvb::protocol::{HostnameSegment, ProxyResolver, UriBundleResolver, UriPathResolver};
pub use wvb::remote::HttpConfig as Http;
pub use wvb::signature::SignatureVerifier;

type SignatureVerifierBuilder =
  Arc<dyn Fn() -> Result<SignatureVerifier, wvb::Error> + Send + Sync>;

#[derive(Clone, Default)]
pub struct Updater {
  pub(crate) channel: Option<String>,
  pub(crate) integrity_policy: Option<IntegrityPolicy>,
  pub(crate) signature_verifier: Option<SignatureVerifierBuilder>,
}

impl Updater {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn channel(mut self, channel: impl Into<String>) -> Self {
    self.channel = Some(channel.into());
    self
  }

  pub fn integrity_policy(mut self, policy: IntegrityPolicy) -> Self {
    self.integrity_policy = Some(policy);
    self
  }

  pub fn signature_verifier<F>(mut self, builder: F) -> Self
  where
    F: Fn() -> Result<SignatureVerifier, wvb::Error> + Send + Sync + 'static,
  {
    self.signature_verifier = Some(Arc::new(builder));
    self
  }

  pub(crate) fn build_config(&self) -> crate::Result<UpdaterConfig> {
    let mut config = UpdaterConfig::default();
    if let Some(ref channel) = self.channel {
      config = config.channel(channel.clone());
    }
    if let Some(policy) = self.integrity_policy {
      config = config.integrity_policy(policy);
    }
    if let Some(ref builder) = self.signature_verifier {
      let verifier = builder()?;
      config = config.signature_verifier(verifier);
    }
    Ok(config)
  }
}

type DynamicDirFn<R> = fn(app: &AppHandle<R>) -> Result<PathBuf, Box<dyn std::error::Error>>;

#[derive(Clone)]
pub(crate) enum Dir<R: Runtime> {
  Static(String),
  Dynamic(DynamicDirFn<R>),
}

impl<R: Runtime> Dir<R> {
  pub fn resolve(&self, app: &AppHandle<R>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    match self {
      Self::Static(dir) => {
        let parsed = app.path().parse(dir)?;
        Ok(parsed)
      }
      Self::Dynamic(f) => {
        let dir = f(app)?;
        Ok(dir)
      }
    }
  }
}

#[derive(Clone, Default)]
pub struct Source<R: Runtime> {
  pub(crate) builtin_dir: Option<Dir<R>>,
  pub(crate) remote_dir: Option<Dir<R>>,
}

impl<R: Runtime> Source<R> {
  pub fn new() -> Self {
    Self {
      builtin_dir: None,
      remote_dir: None,
    }
  }

  pub fn builtin_dir<T: Into<String>>(mut self, dir: T) -> Self {
    self.builtin_dir = Some(Dir::Static(dir.into()));
    self
  }

  pub fn builtin_dir_fn(mut self, dir: DynamicDirFn<R>) -> Self {
    self.builtin_dir = Some(Dir::Dynamic(dir));
    self
  }

  pub fn remote_dir<T: Into<String>>(mut self, dir: T) -> Self {
    self.remote_dir = Some(Dir::Static(dir.into()));
    self
  }

  pub fn remote_dir_fn(mut self, dir: DynamicDirFn<R>) -> Self {
    self.remote_dir = Some(Dir::Dynamic(dir));
    self
  }

  /// Resolves the builtin bundle directory.
  ///
  /// On desktop and iOS the default ([`BaseDirectory::Resource`]) is a real
  /// filesystem path the core can read directly. On **Android** bundled
  /// resources live inside the APK as `asset://` paths that `std::fs` cannot
  /// read, so apps shipping builtin bundles must extract them at startup and
  /// point here via [`Source::builtin_dir_fn`]. Apps that only use remote
  /// (downloaded) bundles are unaffected.
  pub(crate) fn resolve_builtin_dir(&self, app: &AppHandle<R>) -> crate::Result<PathBuf> {
    let dir = match self.builtin_dir {
      Some(ref builtin_dir) => builtin_dir
        .resolve(app)
        .map_err(|e| crate::Error::FailToResolveDirectory(e.to_string()))?,
      None => app.path().resolve("bundles", BaseDirectory::Resource)?,
    };
    Ok(dir)
  }

  pub(crate) fn resolve_remote_dir(&self, app: &AppHandle<R>) -> crate::Result<PathBuf> {
    let dir = match self.remote_dir {
      Some(ref remote_dir) => remote_dir
        .resolve(app)
        .map_err(|e| crate::Error::FailToResolveDirectory(e.to_string()))?,
      None => app.path().resolve("bundles", BaseDirectory::AppLocalData)?,
    };
    Ok(dir)
  }
}

#[derive(Clone, Default)]
pub struct Remote {
  builder: remote::RemoteBuilder,
}

impl Remote {
  pub fn new(endpoint: impl Into<String>) -> Self {
    let builder = remote::Remote::builder().endpoint(endpoint);
    Self { builder }
  }

  pub fn http(mut self, http: Http) -> Self {
    self.builder = self.builder.http(http);
    self
  }

  pub fn on_download<F>(mut self, on_download: F) -> Self
  where
    F: Fn(u64, Option<u64>, String) + Send + Sync + 'static,
  {
    self.builder = self.builder.on_download(on_download);
    self
  }

  pub(crate) fn build(self) -> crate::Result<remote::Remote> {
    let remote = self.builder.build()?;
    Ok(remote)
  }
}

/// Builds the response for a request the protocol failed to serve.
pub type ErrorResponse = Arc<dyn Fn(&crate::Error) -> http::Response<Vec<u8>> + Send + Sync>;

/// A `500` plain-text response, used when a protocol has no [`ErrorResponse`] of its own.
pub fn default_error_response(error: &crate::Error) -> http::Response<Vec<u8>> {
  http::Response::builder()
    .status(http::StatusCode::INTERNAL_SERVER_ERROR)
    .header(http::header::CONTENT_TYPE, "text/plain")
    .body(
      format!("webview bundle protocol error: {error}")
        .as_bytes()
        .to_vec(),
    )
    .expect("static error response")
}

#[derive(Clone)]
pub struct BundleProtocolConfig {
  scheme: String,
  pub(crate) bundle_resolver: Option<UriBundleResolver>,
  pub(crate) path_resolver: Option<UriPathResolver>,
  pub(crate) error_response: Option<ErrorResponse>,
}

impl BundleProtocolConfig {
  pub fn new<S: Into<String>>(scheme: S) -> Self {
    Self {
      scheme: scheme.into(),
      bundle_resolver: None,
      path_resolver: None,
      error_response: None,
    }
  }

  /// The response for a request this protocol fails to serve
  /// (default: [`default_error_response`], a `500` with the message).
  ///
  /// ```no_run
  /// # use tauri::http;
  /// # use wvb_tauri::{Error, Protocol};
  /// Protocol::bundle("app").error_response(|e| {
  ///   let missing = matches!(e, Error::Core(wvb::Error::BundleNotFound));
  ///   http::Response::builder()
  ///     .status(if missing { 404 } else { 500 })
  ///     .body(e.to_string().into_bytes())
  ///     .unwrap()
  /// });
  /// ```
  pub fn error_response<F>(mut self, error_response: F) -> Self
  where
    F: Fn(&crate::Error) -> http::Response<Vec<u8>> + Send + Sync + 'static,
  {
    self.error_response = Some(Arc::new(error_response));
    self
  }

  /// How the bundle name is resolved from the request uri
  /// (default: [`UriBundleResolver::hostname`] with the first hostname segment).
  ///
  /// ```no_run
  /// # use wvb_tauri::{HostnameSegment, Protocol, UriBundleResolver};
  /// Protocol::bundle("app")
  ///   .bundle_resolver(UriBundleResolver::hostname(Some(HostnameSegment::StripSuffix), Some(true)));
  /// ```
  pub fn bundle_resolver(mut self, resolver: UriBundleResolver) -> Self {
    self.bundle_resolver = Some(resolver);
    self
  }

  /// How the file path in the bundle is resolved from the request uri
  /// (default: [`UriPathResolver::directory_index`]).
  ///
  /// ```no_run
  /// # use wvb_tauri::{Protocol, UriPathResolver};
  /// Protocol::bundle("app").path_resolver(UriPathResolver::html_extension());
  /// ```
  pub fn path_resolver(mut self, resolver: UriPathResolver) -> Self {
    self.path_resolver = Some(resolver);
    self
  }
}

#[derive(Clone)]
pub struct ProxyProtocolConfig {
  scheme: String,
  pub(crate) resolver: ProxyResolver,
  pub(crate) max_cache_bytes: Option<usize>,
  pub(crate) error_response: Option<ErrorResponse>,
}

impl ProxyProtocolConfig {
  pub fn new<S: Into<String>>(scheme: S, resolver: ProxyResolver) -> Self {
    Self {
      scheme: scheme.into(),
      resolver,
      max_cache_bytes: None,
      error_response: None,
    }
  }

  /// The response for a request this protocol fails to serve — e.g. a dev server that is not up
  /// yet (default: [`default_error_response`], a `500` with the message).
  pub fn error_response<F>(mut self, error_response: F) -> Self
  where
    F: Fn(&crate::Error) -> http::Response<Vec<u8>> + Send + Sync + 'static,
  {
    self.error_response = Some(Arc::new(error_response));
    self
  }

  /// How many bytes of upstream response bodies the proxy keeps, so an upstream `304 Not Modified`
  /// can be answered with the body last seen for that url (default:
  /// [`wvb::protocol::DEFAULT_MAX_CACHE_BYTES`], 32 MiB; `0` turns the cache off).
  ///
  /// ```no_run
  /// # use wvb_tauri::{Protocol, ProxyResolver};
  /// Protocol::proxy("dev", ProxyResolver::host_mapping([("app", "http://localhost:5173")]))
  ///   .max_cache_bytes(8 * 1024 * 1024);
  /// ```
  pub fn max_cache_bytes(mut self, max_cache_bytes: usize) -> Self {
    self.max_cache_bytes = Some(max_cache_bytes);
    self
  }
}

#[derive(Clone)]
pub enum Protocol {
  Bundle(BundleProtocolConfig),
  Proxy(ProxyProtocolConfig),
}

impl Protocol {
  pub fn bundle<S: Into<String>>(scheme: S) -> BundleProtocolConfig {
    BundleProtocolConfig::new(scheme)
  }

  pub fn proxy<S: Into<String>>(scheme: S, resolver: ProxyResolver) -> ProxyProtocolConfig {
    ProxyProtocolConfig::new(scheme, resolver)
  }

  pub fn scheme(&self) -> &str {
    match self {
      Protocol::Bundle(x) => &x.scheme,
      Protocol::Proxy(x) => &x.scheme,
    }
  }
}

impl From<BundleProtocolConfig> for Protocol {
  fn from(value: BundleProtocolConfig) -> Self {
    Protocol::Bundle(value)
  }
}

impl From<ProxyProtocolConfig> for Protocol {
  fn from(value: ProxyProtocolConfig) -> Self {
    Protocol::Proxy(value)
  }
}

#[derive(Clone, Default)]
pub struct Config<R: Runtime> {
  pub(crate) source: Source<R>,
  pub(crate) protocols: Vec<Protocol>,
  pub(crate) remote: Option<Remote>,
  pub(crate) updater: Option<Updater>,
}

impl<R: Runtime> Config<R> {
  pub fn new() -> Self {
    Self {
      source: Source::new(),
      protocols: vec![],
      remote: Default::default(),
      updater: None,
    }
  }

  pub fn source(mut self, source: Source<R>) -> Self {
    self.source = source;
    self
  }

  pub fn protocol<P: Into<Protocol>>(mut self, protocol: P) -> Self {
    self.protocols.push(protocol.into());
    self
  }

  pub fn remote(mut self, remote: Remote) -> Self {
    self.remote = Some(remote);
    self
  }

  /// Configures updater integrity/signature verification. Requires a remote.
  pub fn updater(mut self, updater: Updater) -> Self {
    self.updater = Some(updater);
    self
  }

  pub(crate) fn build_remote(&self) -> crate::Result<Option<remote::Remote>> {
    if let Some(ref remote_config) = self.remote {
      let remote = remote_config.clone().build()?;
      Ok(Some(remote))
    } else {
      Ok(None)
    }
  }
}
