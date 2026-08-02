use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::AsyncWriteExt;

pub fn normalize_path(base_dir: &Path, path: &Path) -> PathBuf {
  match path.is_absolute() {
    true => path.to_path_buf(),
    false => base_dir.join(path),
  }
}

pub async fn read_file_with_retry(path: &Path) -> std::io::Result<Vec<u8>> {
  let mut attempts = 0;
  loop {
    match tokio::fs::read(path).await {
      Err(e)
      // Windows retries on temporary errors
      // ACCESS_DENIED(5)
      // SHARING_VIOLATION(32)
      if attempts < 20 && cfg!(windows) && matches!(e.raw_os_error(), Some(5) | Some(32)) =>
        {
          attempts += 1;
          tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
      result => return result,
    }
  }
}

static WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

pub async fn atomic_write_file(filepath: &Path, data: &[u8]) -> std::io::Result<()> {
  if let Some(parent) = filepath.parent() {
    let _ = tokio::fs::create_dir_all(parent).await;
  }

  let seq = WRITE_SEQ.fetch_add(1, Ordering::Relaxed);

  let mut tmp = filepath.to_path_buf().into_os_string();
  tmp.push(format!(".{seq}.tmp"));

  let tmp_filepath = Path::new(&tmp);
  let mut tmp_file = tokio::fs::File::create(&tmp).await?;

  if let Err(e) = {
    tmp_file.write_all(data).await?;
    tmp_file.flush().await?;
    drop(tmp_file);
    Ok(())
  } {
    let _ = tokio::fs::remove_file(tmp_filepath).await;
    return Err(e);
  }

  if let Err(e) = rename_with_retry(tmp_filepath, filepath).await {
    let _ = tokio::fs::remove_file(tmp_filepath).await;
    return Err(e);
  }

  if let Some(dir) = filepath.parent() {
    sync_dir(dir).await;
  }

  Ok(())
}

#[cfg(unix)]
async fn sync_dir(dir: &Path) {
  if let Ok(dir) = tokio::fs::File::open(dir).await {
    let _ = dir.sync_all().await;
  }
}

#[cfg(not(unix))]
async fn sync_dir(_dir: &Path) {}

async fn rename_with_retry(from: &Path, to: &Path) -> std::io::Result<()> {
  let mut attempts = 0;
  loop {
    match tokio::fs::rename(from, to).await {
      Err(e)
        if attempts < 20
          && cfg!(windows)
          && matches!(
            e.raw_os_error(),
            // Windows retries on temporary errors
            // ACCESS_DENIED(5)
            // SHARING_VIOLATION(32)
            // ERROR_LOCK_VIOLATION(33)
            // ERROR_USER_MAPPED_FILE(1224)
            Some(5) | Some(32) | Some(33) | Some(1224)
          ) =>
      {
        attempts += 1;
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
      }
      result => return result,
    }
  }
}
