use crate::integrity::IntegrityAlgorithm;
use crate::source::{ManifestBundleSet, ManifestData, Source, SourceOptions};
use crate::testing::{TempDir, TestingBundle, TestingBundleCollection};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn write_bundle_file(filepath: &Path, data: &[u8]) -> anyhow::Result<()> {
  if let Some(dir) = filepath.parent() {
    fs::create_dir_all(dir)?;
  }
  fs::write(filepath, data)?;
  Ok(())
}

#[derive(Debug, Clone)]
enum VersionKind {
  Current,
  Staged,
  Previous,
}

#[derive(Debug, Clone)]
pub struct TestingSourceBuilder {
  temp_dir: TempDir,
  builtin_dir: PathBuf,
  builtin_bundles: TestingBundleCollection,
  remote_dir: PathBuf,
  remote_bundles: TestingBundleCollection,
  remote_versions: HashMap<(String, String), VersionKind>,
  integrity_alg: Option<IntegrityAlgorithm>,
}

impl Default for TestingSourceBuilder {
  fn default() -> Self {
    let temp_dir = TempDir::new();
    let builtin_dir = temp_dir.dir().join("source").join("builtin");
    let remote_dir = temp_dir.dir().join("source").join("remote");

    Self {
      temp_dir,
      builtin_dir,
      builtin_bundles: Default::default(),
      remote_dir,
      remote_bundles: Default::default(),
      remote_versions: Default::default(),
      integrity_alg: None,
    }
  }
}

impl TestingSourceBuilder {
  pub fn new() -> Self {
    Self::default()
  }

  /// The directory the built source reads builtin bundles from, so a test can build a second
  /// source over the same files, as a restarted app would.
  pub fn builtin_dir(&self) -> &Path {
    &self.builtin_dir
  }

  /// The directory the built source reads downloaded bundles from.
  pub fn remote_dir(&self) -> &Path {
    &self.remote_dir
  }

  pub fn add_builtin_bundle(&mut self, bundle: TestingBundle) -> &mut Self {
    // TODO: builtin bundle should have one version per one bundle
    self.builtin_bundles.insert(bundle);
    self
  }

  pub fn add_remote_bundle(&mut self, bundle: TestingBundle) -> &mut Self {
    self.remote_bundles.insert(bundle);
    self
  }

  pub fn set_remote_current_version(
    &mut self,
    bundle_name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    self
      .remote_versions
      .insert((bundle_name.into(), version.into()), VersionKind::Current);
    self
  }

  pub fn set_remote_staged_version(
    &mut self,
    bundle_name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    self
      .remote_versions
      .insert((bundle_name.into(), version.into()), VersionKind::Staged);
    self
  }

  pub fn set_remote_previous_version(
    &mut self,
    bundle_name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    self
      .remote_versions
      .insert((bundle_name.into(), version.into()), VersionKind::Previous);
    self
  }

  pub fn set_integrity_algorithm(&mut self, integrity_algorithm: IntegrityAlgorithm) -> &mut Self {
    self.integrity_alg = Some(integrity_algorithm);
    self
  }

  pub fn build(self) -> anyhow::Result<Source> {
    self.build_with_options(None)
  }

  pub fn build_with_options(self, options: Option<SourceOptions>) -> anyhow::Result<Source> {
    fs::create_dir_all(&self.builtin_dir)?;
    fs::create_dir_all(&self.remote_dir)?;

    let mut builder = Source::builder()
      .builtin_dir(&self.builtin_dir)
      .remote_dir(&self.remote_dir);
    if let Some(options) = options {
      builder = builder.options(options);
    }

    let source = builder.build();
    let mut builtin_manifest = ManifestData::default();

    for bundle in &self.builtin_bundles {
      let filepath = source.get_builtin_bundle_filepath(bundle.name(), bundle.version())?;
      let data = bundle.make_bundle_data()?;
      write_bundle_file(&filepath, &data)?;

      let mut entry = ManifestBundleSet::default();
      entry.versions.insert(
        bundle.version().to_string(),
        bundle.make_version_data(self.integrity_alg)?,
      );
      entry.current_version = Some(bundle.version().to_string());
      builtin_manifest
        .bundles
        .insert(bundle.name().to_string(), entry);
    }

    fs::write(
      source.builtin_manifest.filepath(),
      serde_json::to_string(&builtin_manifest)?,
    )?;

    let mut remote_manifest = ManifestData::default();

    for bundle in &self.remote_bundles {
      let filepath = source.get_remote_bundle_filepath(bundle.name(), bundle.version())?;
      let data = bundle.make_bundle_data()?;
      let version_data = bundle.make_version_data(self.integrity_alg)?;

      write_bundle_file(&filepath, &data)?;

      remote_manifest
        .bundles
        .entry(bundle.name().to_string())
        .and_modify(|entry| {
          entry
            .versions
            .insert(bundle.version().to_string(), version_data.clone());
        })
        .or_insert_with(|| ManifestBundleSet {
          versions: HashMap::from([(bundle.version().to_string(), version_data)]),
          current_version: None,
          previous_version: None,
          staged_version: None,
        });
    }

    for ((bundle_name, version), kind) in self.remote_versions.into_iter() {
      remote_manifest
        .bundles
        .entry(bundle_name)
        .and_modify(|entry| match kind {
          VersionKind::Current => {
            entry.current_version = Some(version);
          }
          VersionKind::Staged => {
            entry.staged_version = Some(version);
          }
          VersionKind::Previous => {
            entry.previous_version = Some(version);
          }
        });
    }

    fs::write(
      self.remote_dir.join(crate::MANIFEST_FILENAME),
      serde_json::to_string(&remote_manifest)?,
    )?;

    Ok(source)
  }
}
