use crate::{Config, Protocol};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Runtime};
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

pub struct WebviewBundle<R: Runtime> {
  _app: AppHandle<R>,
  _config: Arc<Config<R>>,
  source: Arc<BundleSource>,
  remote: Option<Arc<Remote>>,
  updater: Option<Arc<Updater>>,
  protocols: HashMap<String, Arc<dyn protocol::Protocol>>,
}

impl<R: Runtime> WebviewBundle<R> {
  pub(crate) fn init(app: AppHandle<R>, config: Arc<Config<R>>) -> crate::Result<Self> {
    let builtin_dir = config.source.resolve_builtin_dir(&app)?;
    // On Android the resolved dir is an APK `asset://` path the core cannot read
    // with std::fs, so extract the bundles to a real directory first.
    #[cfg(target_os = "android")]
    let builtin_dir = crate::android::extract_builtin_bundles(&app, &builtin_dir)?;
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(builtin_dir.as_path())
        .remote_dir(config.source.resolve_remote_dir(&app)?.as_path())
        .build(),
    );
    let mut protocols = HashMap::with_capacity(config.protocols.len());
    for protocol_config in &config.protocols {
      let scheme = protocol_config.scheme().to_string();
      let protocol: Arc<dyn protocol::Protocol> = match protocol_config {
        Protocol::Bundle(_) => Arc::new(protocol::BundleProtocol::new(source.clone())),
        Protocol::Local(config) => Arc::new(protocol::LocalProtocol::new(config.hosts.clone())),
      };
      if protocols.contains_key(&scheme) {
        return Err(crate::Error::ProtocolSchemeDuplicated { scheme });
      }
      protocols.insert(scheme, protocol);
    }
    let remote = config.build_remote()?.map(Arc::new);
    let updater = remote
      .clone()
      .map(|x| Updater::new(source.clone(), x, None))
      .map(Arc::new);
    Ok(Self {
      _app: app,
      _config: config,
      source,
      remote,
      updater,
      protocols,
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

  pub(crate) fn get_protocol(&self, scheme: &str) -> Option<&Arc<dyn protocol::Protocol>> {
    self.protocols.get(scheme)
  }
}
