mod common;

use common::get;
use http::Request;
use std::sync::Arc;
use wvb::BundleEntry;
use wvb::protocol::{BundleProtocol, Protocol};
use wvb::source::BundleSource;
use wvb::testing::*;
use wvb::updater::Updater;

// Concurrent installs of different bundles take distinct per-bundle locks but share one
// manifest file; without serialized saves an older snapshot could rename last and drop a
// bundle's current_version. A reloaded source must see every activation.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_installs_all_persist() {
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

  // Stage every bundle (no activation yet).
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
    let version = reloaded.get_version(name).await.unwrap().map(|v| v.version);
    assert_eq!(
      version,
      Some("1.0.0".to_string()),
      "bundle {name} lost its activation on disk after concurrent installs"
    );
  }
}

// Re-writing the active version's file while it is served must not tear in-flight reads: the
// write swaps the inode atomically instead of truncating in place.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rewrite_active_bundle_no_torn_reads() {
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
    .get_remote_metadata("app", "1.0.0")
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

// Repeated install swaps under continuous reads. A served response is never torn (the
// descriptor pins its own file). A read MAY fail with a clean BundleNotFound if a rapid install
// pruned a version it had already resolved — never corrupt bytes or a panic.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn repeated_swaps_stay_valid() {
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
      match p.handle(get("https://app.wvb/index.html")).await {
        Ok(resp) => {
          assert_eq!(resp.status(), 200);
          let body = resp.body().as_ref().to_vec();
          assert!(
            allowed.contains(&body),
            "served an unexpected/torn body: {:?}",
            std::str::from_utf8(&body).unwrap_or("<binary>")
          );
        }
        // A version this read resolved was pruned mid-flight — acceptable only as a clean
        // BundleNotFound, never corruption or a panic.
        Err(e) => assert!(
          matches!(e, wvb::Error::BundleNotFound),
          "a failed read must be a clean BundleNotFound, got: {e}"
        ),
      }
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

// A source reloaded while installs run concurrently must never observe a partial/corrupt
// manifest — every parse either succeeds or yields a valid (possibly older) state.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reload_no_partial_manifest() {
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
          // Must never fail with a manifest parse error, regardless of in-flight saves.
          reloaded
            .get_version(name)
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

#[tokio::test]
async fn serve_during_update() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>Version 1.0.0</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>Version 2.0.0</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let remote = Arc::new(system.remote().get_remote());

  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(
    str::from_utf8(resp.body()).unwrap(),
    "<h1>Version 1.0.0</h1>"
  );

  let mut handles = vec![];

  let updater = Updater::new(source.clone(), remote.clone(), None);
  let source_clone = source.clone();
  let update_handle = tokio::spawn(async move {
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    updater.download("app", None).await.unwrap();
    source_clone
      .update_remote_version("app", "2.0.0")
      .await
      .unwrap();
  });

  for _ in 0..50 {
    let protocol = protocol.clone();
    let handle = tokio::spawn(async move {
      tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
      protocol
        .handle(
          Request::builder()
            .uri("https://app.wvb/index.html")
            .method("GET")
            .body(vec![])
            .unwrap(),
        )
        .await
    });
    handles.push(handle);
  }

  update_handle.await.unwrap();

  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    let html = str::from_utf8(resp.body()).unwrap();
    assert!(
      html == "<h1>Version 1.0.0</h1>" || html == "<h1>Version 2.0.0</h1>",
      "Unexpected response: {}",
      html
    );
  }

  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(
    str::from_utf8(resp.body()).unwrap(),
    "<h1>Version 2.0.0</h1>"
  );
}

#[tokio::test]
async fn protocol_updater_stress() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>V1</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>V2</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let remote = Arc::new(system.remote().get_remote());

  let mut handles = vec![];

  for _ in 0..100 {
    let protocol = protocol.clone();
    let handle = tokio::spawn(async move {
      tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
      protocol
        .handle(
          Request::builder()
            .uri("https://app.wvb/index.html")
            .method("GET")
            .body(vec![])
            .unwrap(),
        )
        .await
    });
    handles.push(handle);
  }

  let updater = Updater::new(source.clone(), remote.clone(), None);
  let source_clone = source.clone();
  let update_handle = tokio::spawn(async move {
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    updater.download("app", None).await.unwrap();
    source_clone
      .update_remote_version("app", "2.0.0")
      .await
      .unwrap();
  });

  update_handle.await.unwrap();

  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
  }
}

#[tokio::test]
async fn response_valid_during_swap() {
  // V1/V2 differ in length so a descriptor/file mismatch reads the wrong length and fails LZ4
  // decompression (a detectable error).
  const V1: &[u8] = b"<h1>v1</h1>";
  const V2: &[u8] =
    b"<h1>version 2 - significantly longer content to force different LZ4 size</h1>";

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/index.html", BundleEntry::new(V1, "text/html", None)),
    )
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(
      MockBundle::new("app", "2.0.0")
        .with_entry("/index.html", BundleEntry::new(V2, "text/html", None)),
    )
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let remote = Arc::new(system.remote().get_remote());

  let mut read_handles = vec![];
  for i in 0..200usize {
    let p = protocol.clone();
    let delay_ms = (i % 20) as u64;
    read_handles.push(tokio::spawn(async move {
      tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
      p.handle(
        Request::builder()
          .uri("https://app.wvb/index.html")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
    }));
  }

  let updater = Updater::new(source.clone(), remote.clone(), None);
  tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
  updater.download("app", None).await.unwrap();
  // Activation is the swap the readers race against (download alone only stages v2 on disk).
  source.update_remote_version("app", "2.0.0").await.unwrap();

  for handle in read_handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.body().as_ref();
    assert!(
      body == V1 || body == V2,
      "response body is neither v1 nor v2 — likely a descriptor/file version mismatch:\n  got: {:?}",
      std::str::from_utf8(body).unwrap_or("<binary>")
    );
  }
}

// While install activates a new version, concurrent requests must always get a present, valid
// bundle: the replaced version is retained and each descriptor pins its own file.
#[tokio::test]
async fn install_during_reads_no_missing() {
  const V0: &[u8] = b"<h1>builtin</h1>";
  const V2: &[u8] = b"<h1>v2 - longer body to force a different compressed size</h1>";

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/index.html", BundleEntry::new(V0, "text/html", None)),
    )
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(
      MockBundle::new("app", "2.0.0")
        .with_entry("/index.html", BundleEntry::new(V2, "text/html", None)),
    )
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let updater = Updater::new(source.clone(), remote, None);

  // Stage v2 on disk (not yet active).
  updater.download("app", None).await.unwrap();

  let mut reads = vec![];
  for i in 0..200usize {
    let p = protocol.clone();
    let delay = (i % 20) as u64;
    reads.push(tokio::spawn(async move {
      tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
      p.handle(get("https://app.wvb/index.html")).await
    }));
  }

  tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
  updater.install("app", "2.0.0").await.unwrap();

  for r in reads {
    // Must neither panic nor error.
    let resp = r.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.body().as_ref();
    assert!(
      body == V0 || body == V2,
      "served neither the old nor the new bundle: {:?}",
      std::str::from_utf8(body).unwrap_or("<binary>")
    );
  }
}

// Two installs racing on the same bundle serialize via the per-bundle lock and leave a
// consistent state; the loser may fail because the winner pruned its target.
#[tokio::test]
async fn concurrent_installs_stay_consistent() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>builtin</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = BundleProtocol::new(source.clone());
  let updater = Arc::new(Updater::new(source.clone(), remote, None));

  for v in ["1.1.0", "1.2.0"] {
    system
      .remote_mut()
      .add_bundle(MockBundle::new("app", v).with_entry(
        "/index.html",
        BundleEntry::new(format!("<h1>{v}</h1>").as_bytes(), "text/html", None),
      ))
      .set_bundle_current_version("app", v);
    updater.download("app", None).await.unwrap();
  }

  let u1 = updater.clone();
  let h1 = tokio::spawn(async move { u1.install("app", "1.1.0").await });
  let u2 = updater.clone();
  let h2 = tokio::spawn(async move { u2.install("app", "1.2.0").await });
  let r1 = h1.await.unwrap();
  let r2 = h2.await.unwrap();

  // At least one install wins; the loser may error (its target was pruned by the winner).
  assert!(r1.is_ok() || r2.is_ok(), "both installs failed");

  // Whatever the interleaving, the active version resolves to a present, valid bundle.
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.status(), 200);
  let body = resp.body().as_ref();
  assert!(
    body == b"<h1>1.1.0</h1>" || body == b"<h1>1.2.0</h1>",
    "served neither installed version"
  );
}
