use crate::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_fs::FsExt;

#[derive(serde::Deserialize)]
struct Manifest {
  entries: HashMap<String, ManifestEntry>,
}

#[derive(serde::Deserialize)]
struct ManifestEntry {
  versions: HashMap<String, serde_json::Value>,
}

/// Copies builtin `.wvb` bundles out of the APK's `asset://` resources (which
/// `std::fs` cannot read) into a real filesystem directory the core can serve.
///
/// `resource_dir` is the asset path resolved from [`BaseDirectory::Resource`].
/// Reading the assets goes through the fs plugin, so the host app must register
/// `tauri_plugin_fs::init()`. Returns the extracted directory.
pub(crate) fn extract_builtin_bundles<R: Runtime>(
  app: &AppHandle<R>,
  resource_dir: &Path,
) -> Result<PathBuf> {
  let dest = app.path().resolve("builtin", BaseDirectory::AppLocalData)?;
  let fs = app.fs();

  let manifest_bytes = fs
    .read(resource_dir.join("manifest.json"))
    .map_err(|e| crate::Error::FailToResolveDirectory(format!("read builtin manifest: {e}")))?;

  // Skip re-extraction while the bundled manifest is unchanged; on app update the
  // new manifest differs and everything is re-copied.
  let dest_manifest = dest.join("manifest.json");
  if std::fs::read(&dest_manifest).is_ok_and(|existing| existing == manifest_bytes) {
    return Ok(dest);
  }

  let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
    .map_err(|e| crate::Error::FailToResolveDirectory(format!("parse builtin manifest: {e}")))?;

  for (name, entry) in &manifest.entries {
    let bundle_dest = dest.join(name);
    std::fs::create_dir_all(&bundle_dest)?;
    for version in entry.versions.keys() {
      let filename = format!("{name}_{version}.wvb");
      let bytes = fs
        .read(resource_dir.join(name).join(&filename))
        .map_err(|e| {
          crate::Error::FailToResolveDirectory(format!("read builtin {filename}: {e}"))
        })?;
      std::fs::write(bundle_dest.join(&filename), bytes)?;
    }
  }
  // Written last so it doubles as the completion marker for the skip check above.
  std::fs::write(&dest_manifest, &manifest_bytes)?;
  Ok(dest)
}
