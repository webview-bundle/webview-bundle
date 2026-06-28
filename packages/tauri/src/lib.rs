use std::sync::Arc;
use tauri::{
  Manager, Runtime, UriSchemeContext, http,
  plugin::{Builder, TauriPlugin},
};

pub use config::{
  Config, Http, IntegrityPolicy, Protocol, Remote, SignatureVerifier, Source, Updater,
};
pub use wvb::signature::{
  EcdsaSecp256r1Verifier, EcdsaSecp384r1Verifier, Ed25519Verifier, RsaPkcs1V15Verifier,
  RsaPssVerifier,
};

#[cfg(desktop)]
mod desktop;

mod commands;
mod config;
mod error;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::WebviewBundle;

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
pub fn init<R: Runtime>(config: Config<R>) -> TauriPlugin<R> {
  let config = Arc::new(config);
  let c = config.clone();

  let mut builder = Builder::<R>::new("wvb-tauri").setup(move |app, _api| {
    #[cfg(desktop)]
    let webview_bundle = desktop::init(app, c)?;
    app.manage(webview_bundle);
    Ok(())
  });

  for protocol_config in &config.protocols {
    let scheme = protocol_config.scheme().to_string();
    builder = builder.register_asynchronous_uri_scheme_protocol(
      protocol_config.scheme(),
      move |ctx: UriSchemeContext<R>, req, res| {
        let protocol = ctx
          .app_handle()
          .webview_bundle()
          .get_protocol(&scheme)
          .unwrap_or_else(|| panic!("protocol not found: {scheme}"))
          .clone();
        tauri::async_runtime::spawn(async move {
          match protocol.handle(req).await {
            Ok(resp) => res.respond(resp),
            Err(e) => {
              let resp = http::Response::builder()
                .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                .header(http::header::CONTENT_TYPE, "text/plain")
                .body(
                  format!("webview bundle protocol error: {e}")
                    .as_bytes()
                    .to_vec(),
                )
                .unwrap();
              res.respond(resp);
            }
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
