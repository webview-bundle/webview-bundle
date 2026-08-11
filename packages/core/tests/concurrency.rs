mod common;

use common::{
  builtin_source, bundle, bundle_update, download_all, fetch_update, get, install_all, reload,
  remote_server, source_with_dirs, target, update_all, updater,
};
use std::sync::Arc;
use std::time::Duration;
use wvb::protocol::{BundleProtocol, Protocol};
use wvb::updater::Updater;

// Concurrent installs of different bundles share one manifest file; without serialized saves an
// older snapshot could rename last and drop a bundle's current_version. A reloaded source must
// see every activation.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_installs_all_persist() {
  let names = (0..8).map(|i| format!("app{i}")).collect::<Vec<_>>();
  let served = names
    .iter()
    .map(|name| bundle(name, "1.0.0", format!("<h1>{name}</h1>").as_bytes()))
    .collect();

  let (source, dirs) = source_with_dirs(vec![]);
  let server = remote_server(served);
  let updater = Arc::new(updater(&source, &server.base_url()));

  // Stage every bundle (no activation yet).
  let update = fetch_update(&updater).await;
  download_all(&updater, &update.bundles).await;

  // Activate them all at once.
  let mut handles = vec![];
  for name in &names {
    let updater = updater.clone();
    let name = name.clone();
    handles.push(tokio::spawn(async move {
      updater.install(&[target(&name, "1.0.0")]).await
    }));
  }
  for handle in handles {
    for item in handle.await.unwrap().unwrap() {
      item.result.unwrap();
    }
  }

  // Reload from disk: every activation must have survived.
  let reloaded = reload(&dirs);
  for name in &names {
    let version = reloaded.get_version(name).await.unwrap().map(|x| x.version);
    assert_eq!(
      version,
      Some("1.0.0".to_owned()),
      "bundle {name} lost its activation on disk after concurrent installs"
    );
  }
}

// Re-downloading the active version rewrites its file while it is served; the write swaps the
// file atomically instead of truncating in place, so in-flight reads must not tear.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rewrite_active_bundle_no_torn_reads() {
  const BODY: &[u8] = b"<h1>v1 content long enough to actually exercise lz4 compression</h1>";

  let source = builtin_source(vec![]);
  let server = remote_server(vec![bundle("app", "1.0.0", BODY)]);
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let updater = Arc::new(updater(&source, &server.base_url()));
  update_all(&updater).await;

  let writer = {
    let updater = updater.clone();
    let updates = vec![bundle_update("app", "1.0.0")];
    tokio::spawn(async move {
      for _ in 0..40 {
        updater.download(&updates, None).await.unwrap();
        tokio::time::sleep(Duration::from_millis(1)).await;
      }
    })
  };

  let mut readers = vec![];
  for i in 0..300usize {
    let protocol = protocol.clone();
    let delay = (i % 40) as u64;
    readers.push(tokio::spawn(async move {
      tokio::time::sleep(Duration::from_millis(delay)).await;
      protocol.handle(get("https://app.wvb/index.html")).await
    }));
  }

  writer.await.unwrap();
  for reader in readers {
    let resp = reader.await.unwrap().unwrap();
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

  let source = builtin_source(vec![bundle("app", "1.0.0", V0)]);
  let mut server = remote_server(vec![]);
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let updater = updater(&source, &server.base_url());

  let mut allowed = vec![V0.to_vec()];
  for (_, body) in &versions {
    allowed.push(body.as_bytes().to_vec());
  }

  let mut readers = vec![];
  for i in 0..400usize {
    let protocol = protocol.clone();
    let allowed = allowed.clone();
    let delay = (i % 40) as u64;
    readers.push(tokio::spawn(async move {
      tokio::time::sleep(Duration::from_millis(delay)).await;
      match protocol.handle(get("https://app.wvb/index.html")).await {
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

  for (version, body) in versions {
    server.insert_bundle(bundle("app", version, body.as_bytes()));
    server.set_current_version("app", version);
    update_all(&updater).await;
    tokio::time::sleep(Duration::from_millis(3)).await;
  }

  for reader in readers {
    reader.await.unwrap();
  }
}

// A source reloaded while installs run concurrently must never observe a partial/corrupt
// manifest — every parse either succeeds or yields a valid (possibly older) state.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reload_no_partial_manifest() {
  let names = (0..6).map(|i| format!("app{i}")).collect::<Vec<_>>();
  let served = names
    .iter()
    .map(|name| bundle(name, "1.0.0", b"<h1>x</h1>"))
    .collect();

  let (source, dirs) = source_with_dirs(vec![]);
  let server = remote_server(served);
  let updater = Arc::new(updater(&source, &server.base_url()));
  let update = fetch_update(&updater).await;
  download_all(&updater, &update.bundles).await;

  let mut writers = vec![];
  for name in &names {
    let updater = updater.clone();
    let name = name.clone();
    writers.push(tokio::spawn(async move {
      for item in updater.install(&[target(&name, "1.0.0")]).await.unwrap() {
        item.result.unwrap();
      }
    }));
  }

  // Readers: keep building fresh sources from the same dirs and parsing the manifest.
  let mut readers = vec![];
  for _ in 0..6 {
    let builtin = dirs.builtin.clone();
    let remote = dirs.remote.clone();
    let names = names.clone();
    readers.push(tokio::spawn(async move {
      let dirs = common::SourceDirs { builtin, remote };
      for _ in 0..50 {
        let reloaded = reload(&dirs);
        for name in &names {
          reloaded
            .get_version(name)
            .await
            .expect("fresh source observed a corrupt manifest");
        }
        tokio::task::yield_now().await;
      }
    }));
  }

  for writer in writers {
    writer.await.unwrap();
  }
  for reader in readers {
    reader.await.unwrap();
  }
}

#[tokio::test]
async fn serve_during_update() {
  const V1: &[u8] = b"<h1>Version 1.0.0</h1>";
  const V2: &[u8] = b"<h1>Version 2.0.0</h1>";

  let source = builtin_source(vec![bundle("app", "1.0.0", V1)]);
  let server = remote_server(vec![bundle("app", "2.0.0", V2)]);
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let updater = updater(&source, &server.base_url());

  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), V1);

  let mut handles = vec![];
  for _ in 0..50 {
    let protocol = protocol.clone();
    handles.push(tokio::spawn(async move {
      tokio::time::sleep(Duration::from_millis(5)).await;
      protocol.handle(get("https://app.wvb/index.html")).await
    }));
  }

  tokio::time::sleep(Duration::from_millis(10)).await;
  update_all(&updater).await;

  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.body().as_ref();
    assert!(
      body == V1 || body == V2,
      "unexpected response: {:?}",
      std::str::from_utf8(body).unwrap_or("<binary>")
    );
  }

  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), V2);
}

// Readers race the activation itself: V1/V2 differ in length so a descriptor/file mismatch
// would read the wrong length and fail LZ4 decompression instead of going unnoticed.
#[tokio::test]
async fn response_valid_during_swap() {
  const V1: &[u8] = b"<h1>v1</h1>";
  const V2: &[u8] =
    b"<h1>version 2 - significantly longer content to force different LZ4 size</h1>";

  let source = builtin_source(vec![bundle("app", "1.0.0", V1)]);
  let server = remote_server(vec![bundle("app", "2.0.0", V2)]);
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let updater = updater(&source, &server.base_url());

  let mut readers = vec![];
  for i in 0..200usize {
    let protocol = protocol.clone();
    let delay = (i % 20) as u64;
    readers.push(tokio::spawn(async move {
      tokio::time::sleep(Duration::from_millis(delay)).await;
      protocol.handle(get("https://app.wvb/index.html")).await
    }));
  }

  tokio::time::sleep(Duration::from_millis(5)).await;
  update_all(&updater).await;

  for reader in readers {
    let resp = reader.await.unwrap().unwrap();
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

  let source = builtin_source(vec![bundle("app", "1.0.0", V0)]);
  let server = remote_server(vec![bundle("app", "2.0.0", V2)]);
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let updater = updater(&source, &server.base_url());

  // Stage v2 on disk (not yet active).
  let update = fetch_update(&updater).await;
  download_all(&updater, &update.bundles).await;

  let mut readers = vec![];
  for i in 0..200usize {
    let protocol = protocol.clone();
    let delay = (i % 20) as u64;
    readers.push(tokio::spawn(async move {
      tokio::time::sleep(Duration::from_millis(delay)).await;
      protocol.handle(get("https://app.wvb/index.html")).await
    }));
  }

  tokio::time::sleep(Duration::from_millis(5)).await;
  install_all(&updater, &update.bundles).await;

  for reader in readers {
    let resp = reader.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.body().as_ref();
    assert!(
      body == V0 || body == V2,
      "served neither the old nor the new bundle: {:?}",
      std::str::from_utf8(body).unwrap_or("<binary>")
    );
  }
}

// Two installs racing on the same staged version serialize on the updater lock: one activates
// it, the other finds nothing staged anymore, and the active version stays servable.
#[tokio::test]
async fn concurrent_installs_stay_consistent() {
  let source = builtin_source(vec![bundle("app", "1.0.0", b"<h1>builtin</h1>")]);
  let server = remote_server(vec![bundle("app", "1.1.0", b"<h1>1.1.0</h1>")]);
  let protocol = BundleProtocol::new(source.clone());
  let updater = Arc::new(updater(&source, &server.base_url()));

  let update = fetch_update(&updater).await;
  download_all(&updater, &update.bundles).await;

  let installs = (0..2)
    .map(|_| {
      let updater: Arc<Updater> = updater.clone();
      tokio::spawn(async move { updater.install(&[target("app", "1.1.0")]).await })
    })
    .collect::<Vec<_>>();

  let mut succeeded = 0;
  for install in installs {
    if install.await.unwrap().unwrap()[0].result.is_ok() {
      succeeded += 1;
    }
  }
  assert_eq!(
    succeeded, 1,
    "exactly one install may activate a staged version"
  );

  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.status(), 200);
  assert_eq!(resp.body().as_ref(), b"<h1>1.1.0</h1>");
}
