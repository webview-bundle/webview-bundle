use std::path::{Path, PathBuf};
use wvb_tauri::{Config, Protocol, Source};

/// Resolves the builtin bundle directory for the e2e run.
///
/// The e2e harness packs the Next.js fixture into a directory and points the app at it via the
/// `WVB_E2E_BUNDLES_DIR` environment variable. As a fallback (e.g. a bundled app) it looks for a
/// `bundles` directory next to the executable.
fn resolve_bundles_dir<R: tauri::Runtime>(
  _app: &tauri::AppHandle<R>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
  if let Ok(dir) = std::env::var("WVB_E2E_BUNDLES_DIR") {
    return Ok(PathBuf::from(dir));
  }
  let exe = std::env::current_exe()?;
  let dir = exe.parent().unwrap_or_else(|| Path::new(".")).join("bundles");
  Ok(dir)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(wvb_tauri::init(
      Config::new()
        .source(Source::new().builtin_dir_fn(resolve_bundles_dir))
        .protocol(Protocol::bundle("bundle")),
    ))
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
