use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

pub struct TmpFile {
  filepath: PathBuf,
  tmp_filepath: PathBuf,
}

impl TmpFile {
  pub fn new(filepath: &Path) -> Self {
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut tmp = filepath.to_path_buf().into_os_string();
    tmp.push(format!(".{seq}.tmp"));

    Self {
      filepath: filepath.to_path_buf(),
      tmp_filepath: PathBuf::from(tmp),
    }
  }

  pub fn path(&self) -> &Path {
    self.filepath.as_path()
  }

  pub fn tmp_path(&self) -> &Path {
    self.tmp_filepath.as_path()
  }

  pub async fn commit(&self) -> crate::Result<()> {
    tokio::fs::rename(self.tmp_path(), self.path()).await?;
    Ok(())
  }

  pub fn clear(&self) {
    let _ = std::fs::remove_file(self.path());
  }
}
