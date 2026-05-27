//! Concurrency & atomicity tests for `source` / `updater` / `protocol`.
//!
//! Kept in a separate file from `integration.rs` to isolate the timing-sensitive,
//! multi-threaded cases. Each test targets an invariant that must hold regardless of
//! interleaving:
//!
//! - cross-bundle concurrent installs are durably persisted (no lost activation on disk);
//! - re-writing the active bundle file never tears an in-flight protocol read;
//! - swapping versions under continuous reads never serves a missing/torn bundle;
//! - the on-disk manifest is never observed in a partial/corrupt state.

use http::Request;
use std::sync::Arc;
use wvb::BundleEntry;
use wvb::protocol::{BundleProtocol, Protocol};
use wvb::source::BundleSource;
use wvb::testing::*;
use wvb::updater::Updater;

fn get(uri: &str) -> Request<Vec<u8>> {
  Request::builder()
    .uri(uri)
    .method("GET")
    .body(vec![])
    .unwrap()
}

// Installing several different bundles concurrently must durably persist every
// activation. They take distinct per-bundle locks (so they run in parallel) yet share
// one manifest file; without serialized saves an older snapshot could rename last and
// drop a bundle's current_version on disk. A fresh source reloaded from the same dirs
// must see all of them active.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_cross_bundle_installs_all_persist_durably() {
  let names: Vec<String> = (0..8).map(|i| format!("app{i}")).collect();

  let mut system = MockSystem::new();
  for name in &names {
    system
      .remote_mut()
      .add_bundle(MockBundle::new(name, "1.0.0").with_entry(
        "/index.html",
        BundleEntry::new(format!("<h1>{name}</h1>").as_bytes(), "text/html", None),
      ));
    system
      .remote_mut()
      .set_bundle_current_version(name, "1.0.0");
  }

  let (builtin_dir, remote_dir) = system.source().dirs();
  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let updater = Arc::new(Updater::new(source.clone(), remote, None));

  // Stage every bundle (download only — no activation yet).
  for name in &names {
    updater.download(name.clone(), None).await.unwrap();
  }

  // Activate them all at once.
  let mut handles = vec![];
  for name in &names {
    let u = updater.clone();
    let n = name.clone();
    handles.push(tokio::spawn(async move { u.install(n, "1.0.0").await }));
  }
  for h in handles {
    h.await.unwrap().unwrap();
  }

  // Reload from disk: every activation must have survived.
  let reloaded = BundleSource::builder()
    .builtin_dir(&builtin_dir)
    .remote_dir(&remote_dir)
    .build();
  for name in &names {
    let version = reloaded
      .load_version(name)
      .await
      .unwrap()
      .map(|v| v.version);
    assert_eq!(
      version,
      Some("1.0.0".to_string()),
      "bundle {name} lost its activation on disk after concurrent installs"
    );
  }
}

// Re-writing the file of the currently-active version while it is being served must not
// tear in-flight reads. An in-place truncate would corrupt concurrent readers; the write
// path swaps the inode atomically, so every read sees a complete body.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rewriting_active_bundle_never_tears_concurrent_reads() {
  const BODY: &[u8] = b"<h1>v1 content long enough to actually exercise lz4 compression</h1>";

  let mut system = MockSystem::new();
  system
    .remote_mut()
    .add_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/index.html", BundleEntry::new(BODY, "text/html", None)),
    )
    .set_bundle_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let updater = Updater::new(source.clone(), remote, None);

  updater.download("app", None).await.unwrap();
  updater.install("app", "1.0.0").await.unwrap();

  // The bundle + metadata we keep re-writing into the active version's path.
  let bundle = source.fetch_remote_bundle("app", "1.0.0").await.unwrap();
  let metadata = source
    .load_remote_metadata("app", "1.0.0")
    .await
    .unwrap()
    .unwrap();

  let writer = {
    let source = source.clone();
    tokio::spawn(async move {
      for _ in 0..80 {
        source
          .write_remote_bundle("app", "1.0.0", &bundle, metadata.clone())
          .await
          .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
      }
    })
  };

  let mut readers = vec![];
  for i in 0..300usize {
    let p = protocol.clone();
    let delay = (i % 80) as u64;
    readers.push(tokio::spawn(async move {
      tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
      p.handle(get("https://app.wvb/index.html")).await
    }));
  }

  writer.await.unwrap();
  for r in readers {
    let resp = r.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
      resp.body().as_ref(),
      BODY,
      "a concurrent re-write tore an in-flight read"
    );
  }
}

// Repeated version swaps via install, under continuous concurrent reads, must never
// serve a missing or torn bundle. Each response must parse and equal one of the known
// versions (the descriptor pins its own file, and the previous version is retained).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn repeated_swaps_under_continuous_reads_stay_valid() {
  const V0: &[u8] = b"<h1>builtin v0</h1>";
  let versions = [
    ("1.1.0", "<h1>remote 1.1.0 body</h1>"),
    ("1.2.0", "<h1>remote 1.2.0 a slightly longer body</h1>"),
    ("1.3.0", "<h1>remote 1.3.0 body</h1>"),
  ];

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/index.html", BundleEntry::new(V0, "text/html", None)),
    )
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let updater = Arc::new(Updater::new(source.clone(), remote, None));

  let mut allowed: Vec<Vec<u8>> = vec![V0.to_vec()];
  for (_, body) in &versions {
    allowed.push(body.as_bytes().to_vec());
  }

  // Reader storm.
  let mut readers = vec![];
  for i in 0..400usize {
    let p = protocol.clone();
    let allowed = allowed.clone();
    let delay = (i % 40) as u64;
    readers.push(tokio::spawn(async move {
      tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
      let resp = p.handle(get("https://app.wvb/index.html")).await.unwrap();
      assert_eq!(resp.status(), 200);
      let body = resp.body().as_ref().to_vec();
      assert!(
        allowed.contains(&body),
        "served an unexpected/torn body: {:?}",
        std::str::from_utf8(&body).unwrap_or("<binary>")
      );
    }));
  }

  // Swap driver: download + install each version in turn while reads are in flight.
  let driver = {
    let updater = updater.clone();
    let mut system = system;
    tokio::spawn(async move {
      for (v, body) in versions {
        system
          .remote_mut()
          .add_bundle(MockBundle::new("app", v).with_entry(
            "/index.html",
            BundleEntry::new(body.as_bytes(), "text/html", None),
          ))
          .set_bundle_current_version("app", v);
        updater.download("app", None).await.unwrap();
        updater.install("app", v).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(3)).await;
      }
    })
  };

  driver.await.unwrap();
  for r in readers {
    r.await.unwrap();
  }
}

// A fresh source reloaded from disk while installs/downloads run concurrently must never
// observe a partial/corrupt manifest — every parse either succeeds or yields a valid
// (possibly older) state, never a JSON error.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn manifest_reload_never_observes_partial_writes() {
  let names: Vec<String> = (0..6).map(|i| format!("app{i}")).collect();

  let mut system = MockSystem::new();
  for name in &names {
    system
      .remote_mut()
      .add_bundle(MockBundle::new(name, "1.0.0").with_entry(
        "/index.html",
        BundleEntry::new(b"<h1>x</h1>", "text/html", None),
      ));
    system
      .remote_mut()
      .set_bundle_current_version(name, "1.0.0");
  }

  let (builtin_dir, remote_dir) = system.source().dirs();
  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let updater = Arc::new(Updater::new(source.clone(), remote, None));

  // Writers: stage + activate every bundle concurrently.
  let mut writers = vec![];
  for name in &names {
    let updater = updater.clone();
    let name = name.clone();
    writers.push(tokio::spawn(async move {
      updater.download(name.clone(), None).await.unwrap();
      updater.install(name, "1.0.0").await.unwrap();
    }));
  }

  // Readers: keep building fresh sources from the same dirs and parsing the manifest.
  let mut readers = vec![];
  for _ in 0..6 {
    let builtin_dir = builtin_dir.clone();
    let remote_dir = remote_dir.clone();
    let names = names.clone();
    readers.push(tokio::spawn(async move {
      for _ in 0..50 {
        let reloaded = BundleSource::builder()
          .builtin_dir(&builtin_dir)
          .remote_dir(&remote_dir)
          .build();
        for name in &names {
          // Must never error with a manifest parse error, regardless of in-flight saves.
          reloaded
            .load_version(name)
            .await
            .expect("fresh source observed a corrupt manifest");
        }
        tokio::task::yield_now().await;
      }
    }));
  }

  for w in writers {
    w.await.unwrap();
  }
  for r in readers {
    r.await.unwrap();
  }
}
