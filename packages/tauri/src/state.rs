use crate::config::ErrorResponse;
use crate::{Config, Protocol};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Runtime, http};
use wvb::protocol;
use wvb::remote::Remote;
use wvb::source::BundleSource;
use wvb::updater::Updater;

pub fn init<R: Runtime>(
  app: &AppHandle<R>,
  config: Arc<Config<R>>,
) -> crate::Result<WebviewBundle<R>> {
  let webview_bundle = WebviewBundle::init(app.clone(), config)?;
  Ok(webview_bundle)
}

/// Kept as its concrete type, so the plugin can read back how a protocol resolves a request.
enum ProtocolKind {
  Bundle(Arc<protocol::BundleProtocol>),
  Proxy(Arc<protocol::ProxyProtocol>),
}

struct RegisteredProtocol {
  kind: ProtocolKind,
  error_response: Option<ErrorResponse>,
}

impl RegisteredProtocol {
  fn handler(&self) -> Arc<dyn protocol::Protocol> {
    match &self.kind {
      ProtocolKind::Bundle(protocol) => protocol.clone(),
      ProtocolKind::Proxy(protocol) => protocol.clone(),
    }
  }

  #[cfg(target_os = "android")]
  fn as_bundle(&self) -> Option<&protocol::BundleProtocol> {
    match &self.kind {
      ProtocolKind::Bundle(protocol) => Some(protocol),
      ProtocolKind::Proxy(_) => None,
    }
  }
}

pub struct WebviewBundle<R: Runtime> {
  _app: AppHandle<R>,
  _config: Arc<Config<R>>,
  source: Arc<BundleSource>,
  remote: Option<Arc<Remote>>,
  updater: Option<Arc<Updater>>,
  protocols: HashMap<String, RegisteredProtocol>,
  #[cfg(target_os = "android")]
  builtin_extractor: crate::android::BuiltinExtractor,
}

impl<R: Runtime> WebviewBundle<R> {
  pub(crate) fn init(app: AppHandle<R>, config: Arc<Config<R>>) -> crate::Result<Self> {
    let builtin_dir = config.source.resolve_builtin_dir(&app)?;
    // On Android the resolved dir is an APK `asset://` path the core cannot read
    // with std::fs. The extractor copies the (tiny) manifest now and serves from a
    // real directory; each bundle's `.wvb` files are extracted lazily on first use.
    #[cfg(target_os = "android")]
    let (builtin_dir, builtin_extractor) = {
      let extractor = crate::android::BuiltinExtractor::new(&app, builtin_dir)?;
      (extractor.dest_dir().to_path_buf(), extractor)
    };
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(builtin_dir.as_path())
        .remote_dir(config.source.resolve_remote_dir(&app)?.as_path())
        .build(),
    );
    let mut protocols = HashMap::with_capacity(config.protocols.len());
    for protocol_config in &config.protocols {
      let scheme = protocol_config.scheme().to_string();
      let (kind, error_response) = match protocol_config {
        Protocol::Bundle(config) => {
          let mut bundle = protocol::BundleProtocol::new(source.clone());
          if let Some(resolver) = config.bundle_resolver.clone() {
            bundle = bundle.with_bundle_resolver(resolver);
          }
          if let Some(resolver) = config.path_resolver.clone() {
            bundle = bundle.with_path_resolver(resolver);
          }
          (
            ProtocolKind::Bundle(Arc::new(bundle)),
            config.error_response.clone(),
          )
        }
        Protocol::Proxy(config) => {
          let proxy = protocol::ProxyProtocol::new(config.resolver.clone());
          (
            ProtocolKind::Proxy(Arc::new(proxy)),
            config.error_response.clone(),
          )
        }
      };
      if protocols.contains_key(&scheme) {
        return Err(crate::Error::ProtocolSchemeDuplicated { scheme });
      }
      protocols.insert(
        scheme,
        RegisteredProtocol {
          kind,
          error_response,
        },
      );
    }
    let remote = config.build_remote()?.map(Arc::new);
    let updater = match remote.clone() {
      Some(remote) => {
        let updater_config = match config.updater {
          Some(ref updater) => Some(updater.build_config()?),
          None => None,
        };
        Some(Arc::new(Updater::new(
          source.clone(),
          remote,
          updater_config,
        )))
      }
      None => None,
    };
    Ok(Self {
      _app: app,
      _config: config,
      source,
      remote,
      updater,
      protocols,
      #[cfg(target_os = "android")]
      builtin_extractor,
    })
  }

  pub fn source(&self) -> &Arc<BundleSource> {
    &self.source
  }

  pub fn remote(&self) -> Option<&Arc<Remote>> {
    self.remote.as_ref()
  }

  pub fn updater(&self) -> Option<&Arc<Updater>> {
    self.updater.as_ref()
  }

  pub(crate) fn get_protocol(&self, scheme: &str) -> Option<Arc<dyn protocol::Protocol>> {
    self.protocols.get(scheme).map(RegisteredProtocol::handler)
  }

  /// The response for a request the scheme's protocol failed to serve, from its own
  /// [`error_response`](crate::BundleProtocolConfig::error_response) when it has one.
  pub(crate) fn error_response(
    &self,
    scheme: &str,
    error: &crate::Error,
  ) -> http::Response<Vec<u8>> {
    match self
      .protocols
      .get(scheme)
      .and_then(|protocol| protocol.error_response.as_ref())
    {
      Some(error_response) => error_response(error),
      None => crate::config::default_error_response(error),
    }
  }

  /// Extracts the requested builtin bundle's `.wvb` files before the protocol serves it (Android
  /// only; see [`crate::android::BuiltinExtractor`]). The name comes from the protocol's own
  /// [`bundle_resolver`](protocol::BundleProtocol::bundle_resolver), so extraction and serving
  /// always pick the same bundle.
  #[cfg(target_os = "android")]
  pub(crate) fn ensure_builtin_bundle(
    &self,
    scheme: &str,
    uri: &wvb::http::Uri,
  ) -> crate::Result<()> {
    let Some(bundle) = self
      .protocols
      .get(scheme)
      .and_then(RegisteredProtocol::as_bundle)
    else {
      return Ok(());
    };
    let Some(bundle_name) = bundle.bundle_resolver().resolve(uri) else {
      return Ok(());
    };
    self.builtin_extractor.ensure(&self._app, &bundle_name)
  }
}
