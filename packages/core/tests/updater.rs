mod common;

use common::get;
use http::Request;
use std::sync::Arc;
use wvb::BundleEntry;
use wvb::protocol::{BundleProtocol, Protocol};
use wvb::testing::*;
use wvb::updater::Updater;

#[tokio::test]
async fn concurrent_downloads() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app1", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>App 1 v1</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app1", "1.0.0")
    .add_builtin_bundle(MockBundle::new("app2", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>App 2 v1</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app2", "1.0.0");

  system
    .remote_mut()
    .add_bundle(MockBundle::new("app1", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>App 1 v2</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app1", "2.0.0")
    .add_bundle(MockBundle::new("app2", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>App 2 v2</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app2", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let updater = Arc::new(Updater::new(source.clone(), remote.clone(), None));

  let updater1 = updater.clone();
  let handle1 = tokio::spawn(async move { updater1.download("app1", None).await });

  let updater2 = updater.clone();
  let handle2 = tokio::spawn(async move { updater2.download("app2", None).await });

  handle1.await.unwrap().unwrap();
  handle2.await.unwrap().unwrap();

  source.update_remote_version("app1", "2.0.0").await.unwrap();
  source.update_remote_version("app2", "2.0.0").await.unwrap();

  let protocol = BundleProtocol::new(source.clone());
  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app1.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(str::from_utf8(resp.body()).unwrap(), "<h1>App 1 v2</h1>");

  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app2.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(str::from_utf8(resp.body()).unwrap(), "<h1>App 2 v2</h1>");
}

#[tokio::test]
async fn get_update_unknown_errors() {
  let system = MockSystem::new();
  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let updater = Updater::new(source.clone(), remote.clone(), None);

  let result = updater.get_update("nonexistent").await;
  assert!(result.is_err());
}

#[tokio::test]
async fn update_atomic_under_reads() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>Version 1</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>Version 2</h1>", "text/html", None),
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
  assert_eq!(str::from_utf8(resp.body()).unwrap(), "<h1>Version 1</h1>");

  let updater = Updater::new(source.clone(), remote.clone(), None);
  updater.download("app", None).await.unwrap();
  source.update_remote_version("app", "2.0.0").await.unwrap();

  let mut handles = vec![];
  for _ in 0..50 {
    let protocol = protocol.clone();
    let handle = tokio::spawn(async move {
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

  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(str::from_utf8(resp.body()).unwrap(), "<h1>Version 2</h1>");
  }
}

#[tokio::test]
async fn sequential_updates() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>V1</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = BundleProtocol::new(source.clone());
  let updater = Updater::new(source.clone(), remote.clone(), None);

  for i in 1..=5 {
    let version = format!("1.{}.0", i);

    system
      .remote_mut()
      .add_bundle(MockBundle::new("app", &version).with_entry(
        "/index.html",
        BundleEntry::new(format!("<h1>V{}</h1>", i).as_bytes(), "text/html", None),
      ))
      .set_bundle_current_version("app", &version);

    updater.download("app", None).await.unwrap();
    source.update_remote_version("app", &version).await.unwrap();

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
    assert_eq!(resp.status(), 200);
    let content = str::from_utf8(resp.body()).unwrap();
    assert_eq!(content, format!("<h1>V{}</h1>", i));
  }
}

// A download only stages a version on disk; current_version stays untouched until an explicit
// update_version. Holds for both the first download and re-downloads of an existing entry.
#[tokio::test]
async fn download_stages_without_activating() {
  const V1_CONTENT: &[u8] = b"<h1>v1.1.0</h1>";
  const V2_CONTENT: &[u8] = b"<h1>v2.0.0 - longer content to ensure different LZ4 size</h1>";

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>builtin</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "1.1.0").with_entry(
      "/index.html",
      BundleEntry::new(V1_CONTENT, "text/html", None),
    ))
    .set_bundle_current_version("app", "1.1.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());

  // Stage 1.1.0 and activate it as the baseline.
  Updater::new(source.clone(), remote.clone(), None)
    .download("app", None)
    .await
    .unwrap();
  source.update_remote_version("app", "1.1.0").await.unwrap();

  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(V2_CONTENT, "text/html", None),
    ))
    .set_bundle_current_version("app", "2.0.0");

  let protocol = BundleProtocol::new(source.clone());

  // Re-download stages 2.0.0 but must not switch the active version: 1.1.0 keeps serving.
  Updater::new(source.clone(), remote, None)
    .download("app", None)
    .await
    .unwrap();

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
    resp.body().as_ref(),
    V1_CONTENT,
    "a download alone must not change the active version"
  );

  // Explicit activation switches the protocol to the staged bundle.
  source.update_remote_version("app", "2.0.0").await.unwrap();

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
    resp.body().as_ref(),
    V2_CONTENT,
    "update_version activates the staged bundle"
  );
}

// A 404 from the server must propagate, not be swallowed.
#[tokio::test]
async fn download_unknown_errors() {
  let system = MockSystem::new(); // remote is empty
  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let updater = Updater::new(source, remote, None);

  let err = updater.download("nonexistent", None).await.unwrap_err();
  assert!(
    matches!(err, wvb::Error::RemoteBundleNotFound),
    "expected RemoteBundleNotFound, got: {err}"
  );
}

// A failed download must leave the source in its previous state.
#[tokio::test]
async fn failed_download_preserves_source() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>stable</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  // remote has no "app" bundle -> download fails with RemoteBundleNotFound

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = BundleProtocol::new(source.clone());

  let _ = Updater::new(source.clone(), remote, None)
    .download("app", None)
    .await; // intentionally ignore error

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
  assert_eq!(resp.status(), 200);
  assert_eq!(resp.body().as_ref(), b"<h1>stable</h1>");
}

// download stages on disk without serving it; install activates it.
#[tokio::test]
async fn install_activates_staged_version() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>builtin</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>v2</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = BundleProtocol::new(source.clone());
  let updater = Updater::new(source.clone(), remote, None);

  // Download stages 2.0.0 but the protocol keeps serving the builtin.
  updater.download("app", None).await.unwrap();
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>builtin</h1>");

  // Install activates it.
  updater.install("app", "2.0.0").await.unwrap();
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>v2</h1>");
}

// Installing a version that was never downloaded (no manifest entry) must error.
#[tokio::test]
async fn install_unknown_version_errors() {
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
  let updater = Updater::new(source.clone(), remote, None);

  let err = updater.install("app", "9.9.9").await.unwrap_err();
  assert!(
    matches!(err, wvb::Error::BundleEntryNotExists { .. }),
    "expected BundleEntryNotExists, got: {err}"
  );
}

// Each install keeps {current, previous} and prunes older staged versions; the previous file
// stays on disk for a one-step rollback.
#[tokio::test]
async fn install_prunes_old_and_supports_rollback() {
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
  let updater = Updater::new(source.clone(), remote, None);

  // Download then install, one version at a time.
  for v in ["1.1.0", "1.2.0", "1.3.0"] {
    system
      .remote_mut()
      .add_bundle(MockBundle::new("app", v).with_entry(
        "/index.html",
        BundleEntry::new(format!("<h1>{v}</h1>").as_bytes(), "text/html", None),
      ))
      .set_bundle_current_version("app", v);
    updater.download("app", None).await.unwrap();
    updater.install("app", v).await.unwrap();
    let resp = protocol
      .handle(get("https://app.wvb/index.html"))
      .await
      .unwrap();
    assert_eq!(resp.body().as_ref(), format!("<h1>{v}</h1>").as_bytes());
  }

  // After installing 1.3.0: keep {1.3.0 (current), 1.2.0 (previous)}, prune 1.1.0.
  let mut retained = source.remote_retained_versions("app").await.unwrap();
  retained.sort();
  assert_eq!(retained, vec!["1.2.0".to_string(), "1.3.0".to_string()]);
  assert!(
    source
      .get_remote_metadata("app", "1.1.0")
      .await
      .unwrap()
      .is_none()
  );

  let (_, remote_dir) = system.source().dirs();
  assert!(!remote_dir.join("app").join("app_1.1.0.wvb").exists());
  assert!(remote_dir.join("app").join("app_1.2.0.wvb").exists());
  assert!(remote_dir.join("app").join("app_1.3.0.wvb").exists());

  // Roll back to the retained previous version (its file is still present).
  updater.install("app", "1.2.0").await.unwrap();
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>1.2.0</h1>");
}

// A staged bundle that is corrupt on disk must fail install, leaving the active version intact.
#[tokio::test]
async fn install_rejects_corrupt_on_disk_bundle() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>builtin</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>v2</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = BundleProtocol::new(source.clone());
  let updater = Updater::new(source.clone(), remote, None);

  updater.download("app", None).await.unwrap();

  // Corrupt the staged file on disk.
  let (_, remote_dir) = system.source().dirs();
  std::fs::write(
    remote_dir.join("app").join("app_2.0.0.wvb"),
    b"not a valid wvb file",
  )
  .unwrap();

  let err = updater.install("app", "2.0.0").await.unwrap_err();
  assert!(!matches!(err, wvb::Error::BundleEntryNotExists { .. }));

  // Activation never happened: the builtin is still served.
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>builtin</h1>");
}
