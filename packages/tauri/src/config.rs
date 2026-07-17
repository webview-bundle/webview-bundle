use std::path::PathBuf;
use std::sync::Arc;
use tauri::http;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, Runtime};

#[derive(Clone, Default)]
pub struct UpdaterConfig {
  pub(crate) channel: Option<String>,
  pub(crate) integrity: wvb::updater::UpdaterIntegrityOptions,
  pub(crate) signature: wvb::updater::UpdaterSignatureOptions,
}

impl UpdaterConfig {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn channel(mut self, channel: impl Into<String>) -> Self {
    self.channel = Some(channel.into());
    self
  }

  pub fn integrity(mut self, integrity: wvb::updater::UpdaterIntegrityOptions) -> Self {
    self.integrity = integrity;
    self
  }

  pub fn signature(mut self, signature: wvb::updater::UpdaterSignatureOptions) -> Self {
    self.signature = signature;
    self
  }

  pub(crate) fn build_options(&self) -> wvb::updater::UpdaterOptions {
    let mut options = wvb::updater::UpdaterOptions::default();
    if let Some(ref channel) = self.channel {
      options = options.channel(channel.clone());
    }
    options = options.integrity(self.integrity.clone());
    options = options.signature(self.signature.clone());
    options
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
pub struct SourceConfig<R: Runtime> {
  pub(crate) builtin_dir: Option<Dir<R>>,
  pub(crate) remote_dir: Option<Dir<R>>,
  pub(crate) integrity: wvb::source::BundleSourceIntegrityOptions,
  pub(crate) signature: wvb::source::BundleSourceSignatureOptions,
  pub(crate) header_read: wvb::HeaderReadOptions,
  pub(crate) index_read: wvb::IndexReadOptions,
  pub(crate) data_read: wvb::DataReadOptions,
}

impl<R: Runtime> SourceConfig<R> {
  pub fn new() -> Self {
    Self {
      builtin_dir: None,
      remote_dir: None,
      integrity: Default::default(),
      signature: Default::default(),
      header_read: Default::default(),
      index_read: Default::default(),
      data_read: Default::default(),
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

  /// How bundles are checked against their manifest integrity metadata when they are
  /// loaded from disk.
  pub fn integrity(mut self, integrity: wvb::source::BundleSourceIntegrityOptions) -> Self {
    self.integrity = integrity;
    self
  }

  /// How bundle signatures are verified when bundles are loaded from disk.
  pub fn signature(mut self, signature: wvb::source::BundleSourceSignatureOptions) -> Self {
    self.signature = signature;
    self
  }

  /// How a bundle's header is checked when its descriptor is read on load
  /// (default: checksum verification on, seed `0`).
  pub fn header_read(mut self, options: wvb::HeaderReadOptions) -> Self {
    self.header_read = options;
    self
  }

  /// How a bundle's index is checked when its descriptor is read on load
  /// (default: checksum verification on, seed `0`).
  pub fn index_read(mut self, options: wvb::IndexReadOptions) -> Self {
    self.index_read = options;
    self
  }

  /// How entry data read through this source is checked
  /// (default: checksum verification on, seed `0`).
  ///
  /// This covers every read made through the source, including the entries the bundle
  /// protocol serves.
  pub fn data_read(mut self, options: wvb::DataReadOptions) -> Self {
    self.data_read = options;
    self
  }

  /// Resolves the builtin bundle directory.
  ///
  /// On desktop and iOS the default ([`BaseDirectory::Resource`]) is a real
  /// filesystem path the core can read directly. On **Android** bundled
  /// resources live inside the APK as `asset://` paths that `std::fs` cannot
  /// read, so apps shipping builtin bundles must extract them at startup and
  /// point here via [`SourceConfig::builtin_dir_fn`]. Apps that only use remote
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
pub struct RemoteConfig {
  builder: wvb::remote::RemoteBuilder,
}

impl RemoteConfig {
  pub fn new(endpoint: impl Into<String>) -> Self {
    let builder = wvb::remote::Remote::builder().endpoint(endpoint);
    Self { builder }
  }

  pub fn http(mut self, http: wvb::remote::HttpOptions) -> Self {
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

  pub(crate) fn build(self) -> crate::Result<wvb::remote::Remote> {
    let remote = self.builder.build()?;
    Ok(remote)
  }
}

/// Builds the response for a request the protocol failed to serve.
pub type ErrorResponse = Arc<dyn Fn(&crate::Error) -> http::Response<Vec<u8>> + Send + Sync>;

#[derive(Clone)]
pub struct BundleProtocolConfig {
  scheme: String,
  pub(crate) bundle_resolver: wvb::protocol::UriBundleResolver,
  pub(crate) path_resolver: wvb::protocol::UriPathResolver,
  pub(crate) error_response: Option<ErrorResponse>,
}

impl BundleProtocolConfig {
  pub fn new<S: Into<String>>(scheme: S) -> Self {
    Self {
      scheme: scheme.into(),
      bundle_resolver: Default::default(),
      path_resolver: Default::default(),
      error_response: None,
    }
  }

  /// The response for a request this protocol fails to serve
  /// (default: [`default_error_response`](crate::default_error_response), a `500` with the message).
  ///
  /// ```no_run
  /// # use tauri::http;
  /// # use wvb_tauri::{Error, ProtocolConfig};
  /// ProtocolConfig::bundle("app").error_response(|e| {
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
  /// (default: [`UriBundleResolver::hostname`](wvb::protocol::UriBundleResolver::hostname) with
  /// the first hostname segment).
  ///
  /// ```no_run
  /// # use wvb::protocol::{HostnameSegment, UriBundleResolver};
  /// # use wvb_tauri::ProtocolConfig;
  /// ProtocolConfig::bundle("app")
  ///   .bundle_resolver(UriBundleResolver::hostname(Some(HostnameSegment::StripSuffix), Some(true)));
  /// ```
  pub fn bundle_resolver(mut self, resolver: wvb::protocol::UriBundleResolver) -> Self {
    self.bundle_resolver = resolver;
    self
  }

  /// How the file path in the bundle is resolved from the request uri
  /// (default: [`UriPathResolver::directory_index`](wvb::protocol::UriPathResolver::directory_index)).
  ///
  /// ```no_run
  /// # use wvb::protocol::UriPathResolver;
  /// # use wvb_tauri::ProtocolConfig;
  /// ProtocolConfig::bundle("app").path_resolver(UriPathResolver::html_extension());
  /// ```
  pub fn path_resolver(mut self, resolver: wvb::protocol::UriPathResolver) -> Self {
    self.path_resolver = resolver;
    self
  }
}

#[derive(Clone)]
pub struct ProxyProtocolConfig {
  scheme: String,
  pub(crate) resolver: wvb::protocol::ProxyResolver,
  pub(crate) error_response: Option<ErrorResponse>,
}

impl ProxyProtocolConfig {
  pub fn new<S: Into<String>>(scheme: S, resolver: wvb::protocol::ProxyResolver) -> Self {
    Self {
      scheme: scheme.into(),
      resolver,
      error_response: None,
    }
  }

  /// The response for a request this protocol fails to serve — e.g. a dev server that is not up
  /// yet (default: [`default_error_response`](crate::default_error_response), a `500` with the message).
  pub fn error_response<F>(mut self, error_response: F) -> Self
  where
    F: Fn(&crate::Error) -> http::Response<Vec<u8>> + Send + Sync + 'static,
  {
    self.error_response = Some(Arc::new(error_response));
    self
  }
}

#[derive(Clone)]
pub enum ProtocolConfig {
  Bundle(BundleProtocolConfig),
  Proxy(ProxyProtocolConfig),
}

impl ProtocolConfig {
  pub fn bundle<S: Into<String>>(scheme: S) -> BundleProtocolConfig {
    BundleProtocolConfig::new(scheme)
  }

  pub fn proxy<S: Into<String>>(
    scheme: S,
    resolver: wvb::protocol::ProxyResolver,
  ) -> ProxyProtocolConfig {
    ProxyProtocolConfig::new(scheme, resolver)
  }

  pub fn scheme(&self) -> &str {
    match self {
      ProtocolConfig::Bundle(x) => &x.scheme,
      ProtocolConfig::Proxy(x) => &x.scheme,
    }
  }
}

impl From<BundleProtocolConfig> for ProtocolConfig {
  fn from(value: BundleProtocolConfig) -> Self {
    ProtocolConfig::Bundle(value)
  }
}

impl From<ProxyProtocolConfig> for ProtocolConfig {
  fn from(value: ProxyProtocolConfig) -> Self {
    ProtocolConfig::Proxy(value)
  }
}

#[derive(Clone, Default)]
pub struct Config<R: Runtime> {
  pub(crate) source: SourceConfig<R>,
  pub(crate) protocols: Vec<ProtocolConfig>,
  pub(crate) remote: Option<RemoteConfig>,
  pub(crate) updater: Option<UpdaterConfig>,
}

impl<R: Runtime> Config<R> {
  pub fn new() -> Self {
    Self {
      source: SourceConfig::new(),
      protocols: vec![],
      remote: None,
      updater: None,
    }
  }

  pub fn source(mut self, source: SourceConfig<R>) -> Self {
    self.source = source;
    self
  }

  pub fn protocol<P: Into<ProtocolConfig>>(mut self, protocol: P) -> Self {
    self.protocols.push(protocol.into());
    self
  }

  pub fn remote(mut self, remote: RemoteConfig) -> Self {
    self.remote = Some(remote);
    self
  }

  /// Configures updater integrity/signature verification. Requires a remote.
  pub fn updater(mut self, updater: UpdaterConfig) -> Self {
    self.updater = Some(updater);
    self
  }

  pub(crate) fn build_remote(&self) -> crate::Result<Option<wvb::remote::Remote>> {
    if let Some(ref remote_config) = self.remote {
      let remote = remote_config.clone().build()?;
      Ok(Some(remote))
    } else {
      Ok(None)
    }
  }
}
