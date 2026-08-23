use std::sync::Arc;
use tauri::{
  Manager, Runtime, UriSchemeContext,
  plugin::{Builder, TauriPlugin},
};

pub use config::*;
pub use wvb::signature::{
  EcdsaSecp256r1, EcdsaSecp384r1, Ed25519, RsaPkcs1V15Sha256, RsaPssSha256,
};

#[cfg(target_os = "android")]
pub mod android;
mod command;
mod config;
mod error;
mod state;

pub use error::{Error, Result};
pub use state::default_error_response;
pub use wvb;

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
          tracing::debug!(
            scheme = %scheme,
            method = %req.method(),
            uri = %req.uri(),
            "webview-bundle protocol request"
          );
          let wvb = app.webview_bundle();
          #[cfg(target_os = "android")]
          if let Err(e) = wvb.ensure_builtin_bundle(&scheme, req.uri()).await {
            res.respond(wvb.error_response(&scheme, &e));
            return;
          }
          let protocol = wvb
            .get_protocol(&scheme)
            .unwrap_or_else(|| panic!("protocol not found: {scheme}"));
          match protocol.handle(req).await {
            Ok(resp) => res.respond(resp),
            Err(e) => res.respond(wvb.error_response(&scheme, &Error::Core(e))),
          }
        });
      },
    )
  }
  builder
    .invoke_handler(tauri::generate_handler![
      // source
      command::source::source_list_bundles,
      command::source::source_list_builtin_bundles,
      command::source::source_list_remote_bundles,
      command::source::source_get_version,
      command::source::source_get_remote_staged_version,
      command::source::source_get_remote_previous_version,
      command::source::source_get_builtin_version_data,
      command::source::source_get_remote_version_data,
      command::source::source_update_remote_version,
      command::source::source_update_remote_versions,
      command::source::source_stage_remote_bundle,
      command::source::source_stage_remote_bundles,
      command::source::source_remove_remote_bundle,
      command::source::source_remove_remote_bundles,
      command::source::source_prune_remote_bundle,
      command::source::source_prune_remote_bundles,
      command::source::source_resolve_filepath,
      command::source::source_get_builtin_bundle_filepath,
      command::source::source_get_remote_bundle_filepath,
      command::source::source_unload,
      // remote
      command::remote::remote_get_update,
      command::remote::remote_download,
      // updater
      command::updater::updater_get_update,
      command::updater::updater_download,
      command::updater::updater_install,
      command::updater::updater_rollback,
    ])
    .build()
}
