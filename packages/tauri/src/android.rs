use crate::Result;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
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

/// Lazily extracts builtin `.wvb` bundles out of the APK's `asset://` resources
/// (which `std::fs` cannot read) into a real directory the core can serve.
///
/// The tiny manifest is copied up front so the core can resolve versions, but a
/// bundle's heavier `.wvb` files are copied only when the protocol first serves
/// that bundle — keeping extraction off the app startup path. Reading the assets
/// goes through the fs plugin, so the host app must register
/// `tauri_plugin_fs::init()`.
pub(crate) struct BuiltinExtractor {
  resource_dir: PathBuf,
  dest_dir: PathBuf,
  versions: HashMap<String, Vec<String>>,
  /// Whether `dest_dir` already matches the bundled manifest (unchanged since a
  /// previous launch), so already-extracted `.wvb` files can be reused.
  up_to_date: bool,
  extracted: Mutex<HashSet<String>>,
}

impl BuiltinExtractor {
  /// Copies the builtin manifest into app storage and records its bundles. The
  /// `.wvb` files are left for [`ensure`](Self::ensure).
  pub(crate) fn new<R: Runtime>(app: &AppHandle<R>, resource_dir: PathBuf) -> Result<Self> {
    let dest_dir = app.path().resolve("builtin", BaseDirectory::AppLocalData)?;
    let manifest_bytes = app
      .fs()
      .read(resource_dir.join("manifest.json"))
      .map_err(|e| crate::Error::FailToResolveDirectory(format!("read builtin manifest: {e}")))?;

    let dest_manifest = dest_dir.join("manifest.json");
    let up_to_date = std::fs::read(&dest_manifest).is_ok_and(|existing| existing == manifest_bytes);
    std::fs::create_dir_all(&dest_dir)?;
    if !up_to_date {
      std::fs::write(&dest_manifest, &manifest_bytes)?;
    }

    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
      .map_err(|e| crate::Error::FailToResolveDirectory(format!("parse builtin manifest: {e}")))?;
    let versions = manifest
      .entries
      .into_iter()
      .map(|(name, entry)| (name, entry.versions.into_keys().collect()))
      .collect();

    Ok(Self {
      resource_dir,
      dest_dir,
      versions,
      up_to_date,
      extracted: Mutex::new(HashSet::new()),
    })
  }

  /// The extracted builtin directory the core serves from.
  pub(crate) fn dest_dir(&self) -> &std::path::Path {
    &self.dest_dir
  }

  /// Extracts `bundle_name`'s `.wvb` files on first use. A no-op for non-builtin
  /// bundles, once already extracted this process, or when the files are already
  /// present from an unchanged previous launch.
  pub(crate) fn ensure<R: Runtime>(&self, app: &AppHandle<R>, bundle_name: &str) -> Result<()> {
    let Some(versions) = self.versions.get(bundle_name) else {
      return Ok(());
    };
    let mut extracted = self.extracted.lock().unwrap();
    if extracted.contains(bundle_name) {
      return Ok(());
    }

    let bundle_dest = self.dest_dir.join(bundle_name);
    let already_on_disk = self.up_to_date
      && versions
        .iter()
        .all(|v| bundle_dest.join(format!("{bundle_name}_{v}.wvb")).exists());
    if !already_on_disk {
      let fs = app.fs();
      std::fs::create_dir_all(&bundle_dest)?;
      for version in versions {
        let filename = format!("{bundle_name}_{version}.wvb");
        let bytes = fs
          .read(self.resource_dir.join(bundle_name).join(&filename))
          .map_err(|e| {
            crate::Error::FailToResolveDirectory(format!("read builtin {filename}: {e}"))
          })?;
        std::fs::write(bundle_dest.join(&filename), bytes)?;
      }
    }
    extracted.insert(bundle_name.to_string());
    Ok(())
  }
}
