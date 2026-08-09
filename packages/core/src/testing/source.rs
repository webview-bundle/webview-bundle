use crate::source::{BundleSource, BundleSourceOptions};
use crate::testing::TempDir;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TestingSource {
  _temp_dir: TempDir,
  builtin_dir: PathBuf,
  remote_dir: PathBuf,
}

impl Default for TestingSource {
  fn default() -> Self {
    let temp_dir = TempDir::new();
    let builtin_dir = temp_dir.dir().join("source").join("builtin");
    let remote_dir = temp_dir.dir().join("source").join("remote");
    fs::create_dir_all(&builtin_dir).unwrap();
    fs::create_dir_all(&remote_dir).unwrap();
    Self {
      _temp_dir: temp_dir,
      builtin_dir,
      remote_dir,
    }
  }
}

impl TestingSource {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn source(&self) -> BundleSource {
    self.source_with_options(Default::default())
  }

  pub fn source_with_options(&self, options: BundleSourceOptions) -> BundleSource {
    BundleSource::builder()
      .builtin_dir(&self.builtin_dir)
      .remote_dir(&self.remote_dir)
      .options(options)
      .build()
  }
}
