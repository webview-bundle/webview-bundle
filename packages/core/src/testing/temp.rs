use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use time::{OffsetDateTime, format_description};

static BASE_DIR: OnceLock<PathBuf> = OnceLock::new();

fn base_dir() -> &'static Path {
  BASE_DIR
    .get_or_init(|| {
      let is_ci = std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok();
      if is_ci {
        std::env::temp_dir()
          .join("webview-bundle")
          .join("tests")
          .join("temp")
      } else {
        Path::new(env!("CARGO_MANIFEST_DIR"))
          .join("tests")
          .join("temp")
      }
    })
    .as_ref()
}

pub struct TempDir {
  dir: PathBuf,
}

impl TempDir {
  pub fn new() -> Self {
    Self {
      dir: Self::next_dir_for_today(),
    }
  }

  pub fn dir(&self) -> &Path {
    &self.dir
  }

  fn next_dir_for_today() -> PathBuf {
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    let today = now
      .format(&format_description::parse("[year][month][day]").unwrap())
      .unwrap();
    let date_dir = base_dir().join(&today);
    fs::create_dir_all(&date_dir).unwrap();
    let mut n: u32 = 0;
    loop {
      let count = format!("{n:06}");
      let candidate = date_dir.join(&count);
      match fs::create_dir(&candidate) {
        Ok(()) => return candidate,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
          n = n.saturating_add(1);
          continue;
        }
        Err(e) => panic!("failed to create temp dir: {}", e),
      }
    }
  }
}
