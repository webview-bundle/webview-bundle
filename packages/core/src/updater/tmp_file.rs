use std::path::{Path, PathBuf};

pub struct TmpFile {
  filepath: PathBuf,
}

impl TmpFile {
  pub fn new(real_filepath: &Path, seq: u64) -> Self {
    let mut tmp_filepath = real_filepath.to_path_buf().into_os_string();
    tmp_filepath.push(format!(".{seq}.tmp"));

    Self {
      filepath: tmp_filepath.into(),
    }
  }

  pub fn filepath(&self) -> &Path {
    &self.filepath
  }
}

impl Drop for TmpFile {
  fn drop(&mut self) {
    let _ = std::fs::remove_file(&self.filepath);
  }
}
