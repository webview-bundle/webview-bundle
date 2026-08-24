use crate::Config;
use crate::config::ErrorResponse;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Runtime, http};

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

pub fn init<R: Runtime>(
  app: &AppHandle<R>,
  config: Arc<Config<R>>,
) -> crate::Result<WebviewBundle<R>> {
  let webview_bundle = WebviewBundle::init(app.clone(), config)?;
  Ok(webview_bundle)
}

struct RegisteredProtocol {
  protocol: Arc<dyn wvb::protocol::Protocol>,
  error_response: Option<ErrorResponse>,
  #[cfg(target_os = "android")]
  bundle_resolver: Option<wvb::protocol::UriBundleResolver>,
}

pub struct WebviewBundle<R: Runtime> {
  _app: AppHandle<R>,
  _config: Arc<Config<R>>,
  source: Arc<wvb::source::Source>,
  remote: Option<Arc<wvb::remote::Remote>>,
  updater: Option<Arc<wvb::updater::Updater>>,
  protocols: HashMap<String, RegisteredProtocol>,
  #[cfg(target_os = "android")]
  builtin_extractor: Option<crate::android::AndroidBuiltinExtractor>,
}

impl<R: Runtime> WebviewBundle<R> {
  pub(crate) fn init(app: AppHandle<R>, config: Arc<Config<R>>) -> crate::Result<Self> {
    let source = Arc::new(config.build_source(&app)?);
    #[cfg(target_os = "android")]
    let builtin_extractor = crate::android::AndroidBuiltinExtractor::new(
      &app,
      source.clone(),
      config.builtin_extract_options(),
    )?;
    let remote = config.build_remote()?.map(Arc::new);
    let updater = match remote {
      Some(ref remote) => Some(Arc::new(config.build_updater(&app, &source, remote)?)),
      None => None,
    };

    let mut protocols = HashMap::with_capacity(config.protocols.len());
    for protocol_config in &config.protocols {
      let scheme = protocol_config.scheme().to_string();
      if protocols.contains_key(&scheme) {
        panic!(
          "Protocol scheme duplicated. Only one protocol can be registered for the same scheme."
        );
      }
      protocols.insert(
        scheme,
        RegisteredProtocol {
          protocol: protocol_config.build(&source),
          error_response: protocol_config.error_response(),
          #[cfg(target_os = "android")]
          bundle_resolver: protocol_config.bundle_resolver(),
        },
      );
    }

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

  pub fn source(&self) -> &wvb::source::Source {
    &self.source
  }

  pub fn remote(&self) -> Option<&wvb::remote::Remote> {
    self.remote.as_deref()
  }

  pub fn updater(&self) -> Option<&wvb::updater::Updater> {
    self.updater.as_deref()
  }

  pub(crate) fn get_protocol(&self, scheme: &str) -> Option<Arc<dyn wvb::protocol::Protocol>> {
    self
      .protocols
      .get(scheme)
      .map(|registered| registered.protocol.clone())
  }

  pub(crate) fn error_response(
    &self,
    scheme: &str,
    error: &crate::Error,
  ) -> http::Response<Vec<u8>> {
    match self
      .protocols
      .get(scheme)
      .and_then(|p| p.error_response.as_ref())
    {
      Some(error_response) => error_response(error),
      None => default_error_response(error),
    }
  }

  #[cfg(target_os = "android")]
  pub(crate) async fn ensure_builtin_bundle(
    &self,
    scheme: &str,
    uri: &wvb::http::Uri,
  ) -> crate::Result<()> {
    let Some(extractor) = self.builtin_extractor.as_ref() else {
      return Ok(());
    };
    let Some(bundle_name) = self
      .protocols
      .get(scheme)
      .and_then(|registered| registered.bundle_resolver.as_ref())
      .and_then(|bundle_resolver| bundle_resolver.resolve(uri))
    else {
      return Ok(());
    };
    extractor.ensure(&self._app, bundle_name).await
  }
}
