mod common;

use common::{
  builtin_source, bundle, bundle_update, corrupt, download_all, fetch_update, get, install_all,
  remote_server, target, update_all, updater,
};
use std::sync::Arc;
use wvb::ErrorCode;
use wvb::protocol::{BundleProtocol, Protocol};

#[tokio::test]
async fn updates_every_bundle_of_one_update() {
  let source = builtin_source(vec![
    bundle("app1", "1.0.0", b"<h1>App 1 v1</h1>"),
    bundle("app2", "1.0.0", b"<h1>App 2 v1</h1>"),
  ]);
  let server = remote_server(vec![
    bundle("app1", "2.0.0", b"<h1>App 1 v2</h1>"),
    bundle("app2", "2.0.0", b"<h1>App 2 v2</h1>"),
  ]);

  update_all(&updater(&source, &server.base_url())).await;

  let protocol = BundleProtocol::new(source);
  for (name, body) in [
    ("app1", b"<h1>App 1 v2</h1>".as_slice()),
    ("app2", b"<h1>App 2 v2</h1>".as_slice()),
  ] {
    let resp = protocol
      .handle(get(&format!("https://{name}.wvb/index.html")))
      .await
      .unwrap();
    assert_eq!(resp.body().as_ref(), body);
  }
}

#[tokio::test]
async fn every_read_after_an_update_sees_the_new_version() {
  let source = builtin_source(vec![bundle("app", "1.0.0", b"<h1>Version 1</h1>")]);
  let server = remote_server(vec![bundle("app", "2.0.0", b"<h1>Version 2</h1>")]);
  let protocol = Arc::new(BundleProtocol::new(source.clone()));

  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>Version 1</h1>");

  update_all(&updater(&source, &server.base_url())).await;

  let mut handles = vec![];
  for _ in 0..50 {
    let protocol = protocol.clone();
    handles.push(tokio::spawn(async move {
      protocol.handle(get("https://app.wvb/index.html")).await
    }));
  }
  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.body().as_ref(), b"<h1>Version 2</h1>");
  }
}

#[tokio::test]
async fn sequential_updates() {
  let source = builtin_source(vec![bundle("app", "1.0.0", b"<h1>V0</h1>")]);
  let mut server = remote_server(vec![]);
  let protocol = BundleProtocol::new(source.clone());
  let updater = updater(&source, &server.base_url());

  for i in 1..=5 {
    let version = format!("1.{i}.0");
    let body = format!("<h1>V{i}</h1>");
    server.insert_bundle(bundle("app", &version, body.as_bytes()));
    server.set_current_version("app", &version);

    update_all(&updater).await;

    let resp = protocol
      .handle(get("https://app.wvb/index.html"))
      .await
      .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.body().as_ref(), body.as_bytes());
  }
}

// A download only stages a version on disk; the protocol keeps serving the active one.
#[tokio::test]
async fn download_stages_without_activating() {
  let source = builtin_source(vec![bundle("app", "1.0.0", b"<h1>builtin</h1>")]);
  let server = remote_server(vec![bundle("app", "2.0.0", b"<h1>v2</h1>")]);
  let updater = updater(&source, &server.base_url());
  let protocol = BundleProtocol::new(source.clone());

  let update = fetch_update(&updater).await;
  download_all(&updater, &update.bundles).await;

  assert_eq!(
    source
      .get_remote_staged_version("app")
      .await
      .unwrap()
      .as_deref(),
    Some("2.0.0")
  );
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(
    resp.body().as_ref(),
    b"<h1>builtin</h1>",
    "a download alone must not change the active version"
  );
}

#[tokio::test]
async fn install_activates_the_staged_version() {
  let source = builtin_source(vec![bundle("app", "1.0.0", b"<h1>builtin</h1>")]);
  let server = remote_server(vec![bundle("app", "2.0.0", b"<h1>v2</h1>")]);
  let updater = updater(&source, &server.base_url());
  let protocol = BundleProtocol::new(source.clone());

  let update = fetch_update(&updater).await;
  download_all(&updater, &update.bundles).await;
  install_all(&updater, &update.bundles).await;

  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>v2</h1>");
}

// A 404 from the server must be reported, not swallowed.
#[tokio::test]
async fn download_of_an_unserved_bundle_errors() {
  let source = builtin_source(vec![]);
  let server = remote_server(vec![]);
  let updater = updater(&source, &server.base_url());

  let results = updater
    .download(&[bundle_update("nonexistent", "1.0.0")], None)
    .await
    .unwrap();

  assert_eq!(
    results[0].result.as_ref().unwrap_err().code(),
    ErrorCode::RemoteHttp
  );
}

// A failed download must leave the source in its previous state.
#[tokio::test]
async fn failed_download_preserves_source() {
  let source = builtin_source(vec![bundle("app", "1.0.0", b"<h1>stable</h1>")]);
  let server = remote_server(vec![]);
  let updater = updater(&source, &server.base_url());
  let protocol = BundleProtocol::new(source.clone());

  updater
    .download(&[bundle_update("app", "2.0.0")], None)
    .await
    .unwrap();

  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.status(), 200);
  assert_eq!(resp.body().as_ref(), b"<h1>stable</h1>");
}

// Installing a version that was never downloaded (no manifest entry) must error.
#[tokio::test]
async fn install_of_an_unstaged_version_errors() {
  let source = builtin_source(vec![bundle("app", "1.0.0", b"<h1>builtin</h1>")]);
  let updater = updater(&source, common::OFFLINE_URL);

  let results = updater.install(&[target("app", "9.9.9")]).await.unwrap();

  assert_eq!(
    results[0].result.as_ref().unwrap_err().code(),
    ErrorCode::BundleEntryNotExists
  );
}

// Each install keeps {current, previous} and prunes the versions no longer referenced; the
// previous file stays on disk for a one-step rollback.
#[tokio::test]
async fn install_prunes_old_versions_and_keeps_the_previous_one() {
  let source = builtin_source(vec![bundle("app", "1.0.0", b"<h1>builtin</h1>")]);
  let mut server = remote_server(vec![]);
  let protocol = BundleProtocol::new(source.clone());
  let updater = updater(&source, &server.base_url());

  for version in ["1.1.0", "1.2.0", "1.3.0"] {
    let body = format!("<h1>{version}</h1>");
    server.insert_bundle(bundle("app", version, body.as_bytes()));
    server.set_current_version("app", version);
    update_all(&updater).await;

    let resp = protocol
      .handle(get("https://app.wvb/index.html"))
      .await
      .unwrap();
    assert_eq!(resp.body().as_ref(), body.as_bytes());
  }

  // After installing 1.3.0: keep {1.3.0 (current), 1.2.0 (previous)}, prune 1.1.0.
  let filepath = |version| source.get_remote_bundle_filepath("app", version).unwrap();
  assert!(
    source
      .get_remote_version_data("app", "1.1.0")
      .await
      .unwrap()
      .is_none()
  );
  assert!(!filepath("1.1.0").exists());
  assert!(filepath("1.2.0").exists());
  assert!(filepath("1.3.0").exists());

  // Roll back to the retained previous version: its file is still present.
  source.update_remote_version("app", "1.2.0").await.unwrap();
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>1.2.0</h1>");
}

// A staged bundle that is corrupt on disk must fail install, leaving the active version intact.
#[tokio::test]
async fn install_rejects_a_corrupt_staged_bundle() {
  let source = builtin_source(vec![bundle("app", "1.0.0", b"<h1>builtin</h1>")]);
  let server = remote_server(vec![bundle("app", "2.0.0", b"<h1>v2</h1>")]);
  let updater = updater(&source, &server.base_url());
  let protocol = BundleProtocol::new(source.clone());

  let update = fetch_update(&updater).await;
  download_all(&updater, &update.bundles).await;
  corrupt(
    &source.get_remote_bundle_filepath("app", "2.0.0").unwrap(),
    |data| {
      let mid = data.len() / 2;
      data[mid] ^= 0xff;
    },
  );

  let results = updater.install(&[target("app", "2.0.0")]).await.unwrap();

  assert!(results[0].result.is_err());
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(
    resp.body().as_ref(),
    b"<h1>builtin</h1>",
    "a rejected install must not activate anything"
  );
}
