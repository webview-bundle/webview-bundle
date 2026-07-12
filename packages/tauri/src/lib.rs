use std::sync::Arc;
use tauri::{
  Manager, Runtime, UriSchemeContext, http,
  plugin::{Builder, TauriPlugin},
};

pub use config::{
  Config, HostnameSegment, Http, IntegrityPolicy, Protocol, ProxyResolver, Remote,
  SignatureVerifier, Source, Updater, UriBundleResolver, UriPathResolver,
};
pub use wvb::signature::{
  EcdsaSecp256r1Verifier, EcdsaSecp384r1Verifier, Ed25519Verifier, RsaPkcs1V15Verifier,
  RsaPssVerifier,
};

#[cfg(target_os = "android")]
mod android;
mod commands;
mod config;
mod error;
mod state;

pub use error::{Error, Result};

use state::WebviewBundle;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the tauri APIs.
pub trait WebviewBundleExtra<R: Runtime> {
  fn webview_bundle(&self) -> &WebviewBundle<R>;
  fn wvb(&self) -> &WebviewBundle<R> {
    self.webview_bundle()
  }
}

impl<R: Runtime, T: Manager<R>> WebviewBundleExtra<R> for T {
  fn webview_bundle(&self) -> &WebviewBundle<R> {
    self.state::<WebviewBundle<R>>().inner()
  }
}

/// Initializes the plugin.
///
/// On **Android**, apps that ship builtin bundles must also register
/// `tauri_plugin_fs::init()`: builtin bundles live in the APK as `asset://`
/// resources, and the plugin reads them through the fs plugin to extract them to
/// a readable directory on startup. Desktop, iOS, and remote-only apps are
/// unaffected.
pub fn init<R: Runtime>(config: Config<R>) -> TauriPlugin<R> {
  let config = Arc::new(config);
  let c = config.clone();

  let mut builder = Builder::<R>::new("wvb-tauri").setup(move |app, _api| {
    let webview_bundle = state::init(app, c)?;
    app.manage(webview_bundle);
    Ok(())
  });

  for protocol_config in &config.protocols {
    let scheme = protocol_config.scheme().to_string();
    builder = builder.register_asynchronous_uri_scheme_protocol(
      protocol_config.scheme(),
      move |ctx: UriSchemeContext<R>, req, res| {
        let app = ctx.app_handle().clone();
        let scheme = scheme.clone();
        tauri::async_runtime::spawn(async move {
          // Logs the URI the handler actually receives — useful for confirming the
          // per-platform custom-protocol URL shape (e.g. on mobile).
          tracing::debug!(
            scheme = %scheme,
            method = %req.method(),
            uri = %req.uri(),
            "webview-bundle protocol request"
          );
          let wvb = app.webview_bundle();
          // Android serves builtin bundles from extracted assets, so copy the
          // requested bundle out (if not already) before the protocol reads it.
          #[cfg(target_os = "android")]
          if let Err(e) = wvb.ensure_builtin_bundle(&scheme, req.uri()) {
            res.respond(protocol_error_response(&e));
            return;
          }
          let protocol = wvb
            .get_protocol(&scheme)
            .unwrap_or_else(|| panic!("protocol not found: {scheme}"))
            .clone();
          match protocol.handle(req).await {
            Ok(resp) => res.respond(resp),
            Err(e) => res.respond(protocol_error_response(&e)),
          }
        });
      },
    )
  }
  builder
    .invoke_handler(tauri::generate_handler![
      // source
      commands::source_list_bundles,
      commands::source_load_version,
      commands::source_update_version,
      commands::source_resolve_filepath,
      commands::source_get_builtin_bundle_filepath,
      commands::source_get_remote_bundle_filepath,
      commands::source_load_builtin_metadata,
      commands::source_load_remote_metadata,
      commands::source_unload_descriptor,
      commands::source_remove_remote_bundle,
      commands::source_remote_retained_versions,
      commands::source_prune_remote_bundles,
      // remote
      commands::remote_list_bundles,
      commands::remote_get_info,
      commands::remote_download,
      commands::remote_download_version,
      // updater
      commands::updater_list_remotes,
      commands::updater_get_update,
      commands::updater_download,
      commands::updater_install,
    ])
    .build()
}

/// A `500` plain-text response for a failed protocol request.
fn protocol_error_response(error: &dyn std::fmt::Display) -> http::Response<Vec<u8>> {
  http::Response::builder()
    .status(http::StatusCode::INTERNAL_SERVER_ERROR)
    .header(http::header::CONTENT_TYPE, "text/plain")
    .body(
      format!("webview bundle protocol error: {error}")
        .as_bytes()
        .to_vec(),
    )
    .unwrap()
}
