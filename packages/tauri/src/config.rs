use std::path::PathBuf;
use std::sync::Arc;
use tauri::http;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, Runtime};

type DynamicPathFn<R> = fn(app: &AppHandle<R>) -> Result<PathBuf, Box<dyn std::error::Error>>;

#[derive(Clone)]
pub(crate) enum Path<R: Runtime> {
  Static(String),
  Dynamic(DynamicPathFn<R>),
}

impl<R: Runtime> Path<R> {
  pub fn resolve(&self, app: &AppHandle<R>) -> crate::Result<PathBuf> {
    match self {
      Self::Static(dir) => {
        let parsed = app.path().parse(dir)?;
        Ok(parsed)
      }
      Self::Dynamic(f) => {
        let dir = f(app).map_err(|e| crate::Error::FailToResolvePath(e.to_string()))?;
        Ok(dir)
      }
    }
  }
}

#[derive(Clone)]
pub struct UpdaterConfig<R: Runtime> {
  pub(crate) update_filepath: Option<Path<R>>,
  pub(crate) channel: Option<String>,
  pub(crate) integrity: Option<wvb::updater::UpdaterIntegrityOptions>,
  pub(crate) signature: Option<wvb::updater::UpdaterSignatureOptions>,
}

impl<R: Runtime> UpdaterConfig<R> {
  pub fn new() -> Self {
    Self {
      update_filepath: None,
      channel: None,
      integrity: None,
      signature: None,
    }
  }

  pub fn update_filepath<T: Into<String>>(mut self, filepath: T) -> Self {
    self.update_filepath = Some(Path::Static(filepath.into()));
    self
  }

  pub fn update_filepath_fn(mut self, filepath_fn: DynamicPathFn<R>) -> Self {
    self.update_filepath = Some(Path::Dynamic(filepath_fn));
    self
  }

  pub fn channel(mut self, channel: impl Into<String>) -> Self {
    self.channel = Some(channel.into());
    self
  }

  pub fn integrity(mut self, integrity: wvb::updater::UpdaterIntegrityOptions) -> Self {
    self.integrity = Some(integrity);
    self
  }

  pub fn signature(mut self, signature: wvb::updater::UpdaterSignatureOptions) -> Self {
    self.signature = Some(signature);
    self
  }
}

impl<R: Runtime> From<&UpdaterConfig<R>> for wvb::updater::UpdaterOptions {
  fn from(value: &UpdaterConfig<R>) -> Self {
    let mut options = wvb::updater::UpdaterOptions::default();
    if let Some(ref channel) = value.channel {
      options = options.channel(channel.clone());
    }
    if let Some(integrity) = &value.integrity {
      options = options.integrity(integrity.clone());
    }
    if let Some(signature) = &value.signature {
      options = options.signature(signature.clone());
    }
    options
  }
}

#[derive(Clone)]
pub struct SourceConfig<R: Runtime> {
  pub(crate) builtin_dir: Option<Path<R>>,
  pub(crate) builtin_manifest_filepath: Option<Path<R>>,
  pub(crate) remote_dir: Option<Path<R>>,
  pub(crate) remote_manifest_filepath: Option<Path<R>>,
  pub(crate) integrity: Option<wvb::source::SourceIntegrityOptions>,
  pub(crate) header_read: Option<wvb::HeaderReadOptions>,
  pub(crate) index_read: Option<wvb::IndexReadOptions>,
  pub(crate) data_read: Option<wvb::DataReadOptions>,
  pub(crate) remove_bundle_chunk_size: Option<usize>,
}

impl<R: Runtime> SourceConfig<R> {
  pub fn new() -> Self {
    Self {
      builtin_dir: None,
      builtin_manifest_filepath: None,
      remote_dir: None,
      remote_manifest_filepath: None,
      integrity: None,
      header_read: None,
      index_read: None,
      data_read: None,
      remove_bundle_chunk_size: None,
    }
  }

  pub fn builtin_dir<T: Into<String>>(mut self, dir: T) -> Self {
    self.builtin_dir = Some(Path::Static(dir.into()));
    self
  }

  pub fn builtin_dir_fn(mut self, dir: DynamicPathFn<R>) -> Self {
    self.builtin_dir = Some(Path::Dynamic(dir));
    self
  }

  pub fn builtin_manifest_filepath<T: Into<String>>(mut self, filepath: T) -> Self {
    self.builtin_manifest_filepath = Some(Path::Static(filepath.into()));
    self
  }

  pub fn builtin_manifest_filepath_fn(mut self, filepath: DynamicPathFn<R>) -> Self {
    self.builtin_manifest_filepath = Some(Path::Dynamic(filepath));
    self
  }

  pub fn remote_dir<T: Into<String>>(mut self, dir: T) -> Self {
    self.remote_dir = Some(Path::Static(dir.into()));
    self
  }

  pub fn remote_dir_fn(mut self, dir: DynamicPathFn<R>) -> Self {
    self.remote_dir = Some(Path::Dynamic(dir));
    self
  }

  pub fn remote_manifest_filepath<T: Into<String>>(mut self, filepath: T) -> Self {
    self.remote_manifest_filepath = Some(Path::Static(filepath.into()));
    self
  }

  pub fn remote_manifest_filepath_fn(mut self, filepath: DynamicPathFn<R>) -> Self {
    self.remote_manifest_filepath = Some(Path::Dynamic(filepath));
    self
  }

  /// How bundles are checked against their manifest integrity metadata when they are
  /// loaded from disk.
  pub fn integrity(mut self, integrity: wvb::source::SourceIntegrityOptions) -> Self {
    self.integrity = Some(integrity);
    self
  }

  /// How a bundle's header is checked when its descriptor is read on load
  /// (default: checksum verification on, seed `0`).
  pub fn header_read(mut self, options: wvb::HeaderReadOptions) -> Self {
    self.header_read = Some(options);
    self
  }

  /// How a bundle's index is checked when its descriptor is read on load
  /// (default: checksum verification on, seed `0`).
  pub fn index_read(mut self, options: wvb::IndexReadOptions) -> Self {
    self.index_read = Some(options);
    self
  }

  /// How entry data read through this source is checked
  /// (default: checksum verification on, seed `0`).
  ///
  /// This covers every read made through the source, including the entries the bundle
  /// protocol serves.
  pub fn data_read(mut self, options: wvb::DataReadOptions) -> Self {
    self.data_read = Some(options);
    self
  }

  pub fn remove_bundle_chunk_size(mut self, size: usize) -> Self {
    self.remove_bundle_chunk_size = Some(size);
    self
  }
}

impl<R: Runtime> From<&SourceConfig<R>> for wvb::source::SourceOptions {
  fn from(value: &SourceConfig<R>) -> Self {
    let mut options = wvb::source::SourceOptions::default();
    if let Some(integrity) = &value.integrity {
      options = options.integrity(integrity.clone());
    }
    if let Some(header_read) = &value.header_read {
      options = options.header_read(*header_read);
    }
    if let Some(index_read) = &value.index_read {
      options = options.index_read(*index_read);
    }
    if let Some(data_read) = &value.data_read {
      options = options.data_read(*data_read);
    }
    if let Some(size) = value.remove_bundle_chunk_size {
      options = options.remove_bundle_chunk_size(size);
    }
    options
  }
}

#[derive(Clone, Default)]
pub struct RemoteConfig {
  builder: wvb::remote::RemoteBuilder,
}

impl RemoteConfig {
  pub fn new(base_url: impl Into<String>) -> Self {
    let builder = wvb::remote::Remote::builder().base_url(base_url);
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
  pub(crate) bundle_resolver: Option<wvb::protocol::UriBundleResolver>,
  pub(crate) path_resolver: Option<wvb::protocol::UriPathResolver>,
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
    self.bundle_resolver = Some(resolver);
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
    self.path_resolver = Some(resolver);
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

  pub(crate) fn error_response(&self) -> Option<ErrorResponse> {
    match self {
      ProtocolConfig::Bundle(x) => x.error_response.clone(),
      ProtocolConfig::Proxy(x) => x.error_response.clone(),
    }
  }

  #[cfg(target_os = "android")]
  pub(crate) fn bundle_resolver(&self) -> Option<wvb::protocol::UriBundleResolver> {
    match self {
      ProtocolConfig::Bundle(x) => Some(x.bundle_resolver.clone().unwrap_or_default()),
      ProtocolConfig::Proxy(_) => None,
    }
  }

  pub(crate) fn build(
    &self,
    source: &Arc<wvb::source::Source>,
  ) -> Arc<dyn wvb::protocol::Protocol> {
    match self {
      Self::Bundle(config) => {
        let mut p = wvb::protocol::BundleProtocol::new(source.clone());
        if let Some(resolver) = &config.bundle_resolver {
          p = p.set_bundle_resolver(resolver.clone());
        }
        if let Some(resolver) = &config.path_resolver {
          p = p.set_path_resolver(resolver.clone());
        }
        Arc::new(p)
      }
      Self::Proxy(config) => Arc::new(wvb::protocol::ProxyProtocol::new(config.resolver.clone())),
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
  pub(crate) source: Option<SourceConfig<R>>,
  pub(crate) protocols: Vec<ProtocolConfig>,
  pub(crate) remote: Option<RemoteConfig>,
  pub(crate) updater: Option<UpdaterConfig<R>>,
  #[cfg(target_os = "android")]
  pub(crate) android: Option<crate::android::AndroidOptions>,
}

impl<R: Runtime> Config<R> {
  pub fn new() -> Self {
    Self {
      source: None,
      protocols: vec![],
      remote: None,
      updater: None,
      #[cfg(target_os = "android")]
      android: None,
    }
  }

  pub fn source(mut self, source: SourceConfig<R>) -> Self {
    self.source = Some(source);
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
  pub fn updater(mut self, updater: UpdaterConfig<R>) -> Self {
    self.updater = Some(updater);
    self
  }

  #[cfg(target_os = "android")]
  pub fn android(mut self, android: crate::android::AndroidOptions) -> Self {
    self.android = Some(android);
    self
  }

  pub(crate) fn build_source(&self, app: &AppHandle<R>) -> crate::Result<wvb::source::Source> {
    let builtin_dir = if let Some(ref source) = self.source
      && let Some(ref dir) = source.builtin_dir
    {
      dir.resolve(app)
    } else {
      #[cfg(target_os = "android")]
      let dir = app.path().resolve("builtin", BaseDirectory::AppData);
      #[cfg(not(target_os = "android"))]
      let dir = app.path().resolve("bundles", BaseDirectory::Resource);
      dir.map_err(Into::into)
    }?;

    let builtin_manifest_filepath = &self
      .source
      .as_ref()
      .and_then(|x| x.builtin_manifest_filepath.as_ref().map(|p| p.resolve(app)))
      .transpose()?;

    let remote_dir = if let Some(ref source) = self.source
      && let Some(ref dir) = source.remote_dir
    {
      dir.resolve(app)
    } else {
      app
        .path()
        .resolve("bundles", BaseDirectory::AppData)
        .map_err(Into::into)
    }?;
    let remote_manifest_filepath = &self
      .source
      .as_ref()
      .and_then(|x| x.remote_manifest_filepath.as_ref().map(|p| p.resolve(app)))
      .transpose()?;

    let mut builder = wvb::source::Source::builder()
      .builtin_dir(builtin_dir)
      .remote_dir(remote_dir);

    if let Some(filepath) = builtin_manifest_filepath {
      builder = builder.builtin_manifest_filepath(filepath);
    }
    if let Some(filepath) = remote_manifest_filepath {
      builder = builder.remote_manifest_filepath(filepath);
    }
    if let Some(config) = &self.source {
      builder = builder.options(wvb::source::SourceOptions::from(config));
    }

    let source = builder.build();
    Ok(source)
  }

  pub(crate) fn build_remote(&self) -> crate::Result<Option<wvb::remote::Remote>> {
    if let Some(ref remote_config) = self.remote {
      let remote = remote_config.clone().build()?;
      Ok(Some(remote))
    } else {
      Ok(None)
    }
  }

  pub(crate) fn build_updater(
    &self,
    app: &AppHandle<R>,
    source: &Arc<wvb::source::Source>,
    remote: &Arc<wvb::remote::Remote>,
  ) -> crate::Result<wvb::updater::Updater> {
    let update_filepath = if let Some(ref updater) = self.updater
      && let Some(ref filepath) = updater.update_filepath
    {
      filepath.resolve(app)
    } else {
      app
        .path()
        .resolve("update.json", BaseDirectory::AppData)
        .map_err(Into::into)
    }?;
    let mut builder = wvb::updater::Updater::builder()
      .source(source.clone())
      .remote(remote.clone())
      .update_filepath(&update_filepath);
    if let Some(config) = &self.updater {
      builder = builder.options(config.into());
    }

    let updater = builder.build()?;
    Ok(updater)
  }
}
