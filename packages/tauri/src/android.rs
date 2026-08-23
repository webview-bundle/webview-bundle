use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::async_runtime::Mutex;
use tauri::{AppHandle, Runtime};
use tauri_plugin_fs::FsExt;

const ASSET_URI_PREFIX: &str = "asset://localhost/";
const DEFAULT_ASSET_DIR: &str = "bundles";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct AndroidOptions {
  pub builtin_extract: Option<AndroidBuiltinExtractOptions>,
}

impl AndroidOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn builtin_extract(mut self, builtin_extract: AndroidBuiltinExtractOptions) -> Self {
    self.builtin_extract = Some(builtin_extract);
    self
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AndroidBuiltinExtractOptions {
  Enabled {
    /// e.g. bundles/
    asset_dir: Option<String>,
    asset_manifest_filepath: Option<String>,
  },
  Disabled,
}

impl Default for AndroidBuiltinExtractOptions {
  fn default() -> Self {
    Self::Enabled {
      asset_dir: None,
      asset_manifest_filepath: None,
    }
  }
}

impl AndroidBuiltinExtractOptions {
  fn asset_paths(&self) -> Option<(PathBuf, PathBuf)> {
    let Self::Enabled {
      asset_dir,
      asset_manifest_filepath,
    } = self
    else {
      return None;
    };
    let dir = asset_dir
      .as_deref()
      .map(trim_asset_path)
      .unwrap_or(DEFAULT_ASSET_DIR);
    let manifest_filepath = asset_manifest_filepath
      .as_deref()
      .map(|x| trim_asset_path(x).to_string())
      .unwrap_or_else(|| format!("{dir}/{}", wvb::MANIFEST_FILENAME));
    Some((asset_path(dir), asset_path(&manifest_filepath)))
  }
}

fn trim_asset_path(path: &str) -> &str {
  path.trim_matches('/')
}

fn asset_path(path: &str) -> PathBuf {
  PathBuf::from(format!("{ASSET_URI_PREFIX}{path}"))
}

pub(crate) struct AndroidBuiltinExtractor {
  source: Arc<wvb::source::Source>,
  asset_dir: PathBuf,
  extracted: Mutex<HashSet<String>>,
}

impl AndroidBuiltinExtractor {
  pub(crate) fn new<R: Runtime>(
    app: &AppHandle<R>,
    source: impl Into<Arc<wvb::source::Source>>,
    options: AndroidBuiltinExtractOptions,
  ) -> crate::Result<Option<Self>> {
    let Some((asset_dir, asset_manifest_filepath)) = options.asset_paths() else {
      return Ok(None);
    };
    let source = source.into();
    let Ok(manifest) = app.fs().read(&asset_manifest_filepath) else {
      return Ok(None);
    };

    let manifest_filepath = source.builtin_manifest().filepath();
    if !std::fs::read(manifest_filepath).is_ok_and(|extracted| extracted == manifest) {
      let _ = std::fs::remove_dir_all(source.builtin_dir());
      std::fs::create_dir_all(source.builtin_dir())?;
      if let Some(parent) = manifest_filepath.parent() {
        std::fs::create_dir_all(parent)?;
      }
      std::fs::write(manifest_filepath, manifest)?;
    }

    Ok(Some(Self {
      source,
      asset_dir,
      extracted: Mutex::new(HashSet::new()),
    }))
  }

  pub(crate) async fn ensure<R: Runtime>(
    &self,
    app: &AppHandle<R>,
    bundle_name: impl Into<String>,
  ) -> crate::Result<()> {
    let bundle_name = bundle_name.into();

    let mut extracted = self.extracted.lock().await;
    if extracted.contains(&bundle_name) {
      return Ok(());
    }

    let versions = self
      .source
      .list_builtin_bundles()
      .await?
      .into_iter()
      .filter(|x| x.item.name == bundle_name)
      .map(|x| x.item.version)
      .collect::<Vec<_>>();

    for version in versions {
      let dest = self
        .source
        .get_builtin_bundle_filepath(&bundle_name, &version)?;
      if dest.exists() {
        continue;
      }
      let asset = self.asset_filepath(&dest)?;
      let bundle = app.fs().read(&asset)?;
      write_bundle(&dest, &bundle)?;
    }
    extracted.insert(bundle_name);
    Ok(())
  }

  fn asset_filepath(&self, dest: &Path) -> crate::Result<PathBuf> {
    let relative = dest
      .strip_prefix(self.source.builtin_dir())
      .map_err(|_| crate::Error::FailToResolvePath(dest.to_string_lossy().to_string()))?;
    Ok(self.asset_dir.join(relative))
  }
}

fn write_bundle(dest: &Path, bundle: &[u8]) -> crate::Result<()> {
  if let Some(parent) = dest.parent() {
    let _ = std::fs::create_dir_all(parent);
  }
  let temp = dest.with_extension("tmp");
  std::fs::write(&temp, bundle)?;
  std::fs::rename(&temp, dest)?;
  Ok(())
}
