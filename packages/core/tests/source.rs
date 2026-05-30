use http::Request;
use std::sync::Arc;
use wvb::BundleEntry;
use wvb::protocol::{BundleProtocol, Protocol};
use wvb::source::BundleSource;
use wvb::testing::*;
use wvb::updater::Updater;

#[tokio::test]
async fn manifest_persists_across_reload() {
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
      BundleEntry::new(b"<h1>remote</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app", "2.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();
  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());

  Updater::new(source.clone(), remote, None)
    .download("app", None)
    .await
    .unwrap();
  // Activate so current_version is persisted to the on-disk manifest.
  source.update_remote_version("app", "2.0.0").await.unwrap();

  // Simulate a restart with a fresh source over the same dirs.
  let reloaded = Arc::new(
    BundleSource::builder()
      .builtin_dir(builtin_dir)
      .remote_dir(remote_dir)
      .build(),
  );
  let version = reloaded.load_version("app").await.unwrap().unwrap();
  assert_eq!(
    version.version, "2.0.0",
    "downloaded version must survive a source reload"
  );

  let protocol = BundleProtocol::new(reloaded);
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
  assert_eq!(resp.body().as_ref(), b"<h1>remote</h1>");
}

// V1/V2 differ in length on purpose: equal LZ4 sizes would let a stale v1 descriptor read
// the v2 file with the right byte count and hide the bug.
#[tokio::test]
async fn descriptor_cache_invalidated_on_activation() {
  const V1: &[u8] = b"<h1>v1</h1>";
  const V2: &[u8] = b"<h1>version 2 - longer content ensures different LZ4 compressed size</h1>";

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
  let remote = Arc::new(system.remote().get_remote());
  let protocol = BundleProtocol::new(source.clone());

  // Warm the descriptor cache with v1.
  let warm = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(warm.body().as_ref(), V1);

  // Stage v2, then activate it.
  Updater::new(source.clone(), remote, None)
    .download("app", None)
    .await
    .unwrap();
  // Activation changes the active filepath; a stale v1 descriptor would mis-read the v2 file.
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
    V2,
    "descriptor cache was not invalidated after activation"
  );
}

// A descriptor/file version mismatch must hard-error, not silently return wrong bytes.
#[tokio::test]
async fn descriptor_file_mismatch_errors() {
  use tokio::fs::File;
  use wvb::source::BundleSource;

  const V1: &[u8] = b"<h1>v1</h1>";
  const V2: &[u8] = b"<h1>version 2 - longer content to guarantee different LZ4 size</h1>";

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/index.html", BundleEntry::new(V1, "text/html", None)),
    )
    .set_builtin_current_version("app", "1.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();

  // Write the v2 file directly so the manifest still points at v1 (present but inactive).
  let v2_bundle = MockBundle::new("app", "2.0.0")
    .with_entry("/index.html", BundleEntry::new(V2, "text/html", None));
  let v2_dir = remote_dir.join("app");
  std::fs::create_dir_all(&v2_dir).unwrap();
  let v2_path = v2_dir.join("app_2.0.0.wvb");
  std::fs::write(&v2_path, v2_bundle.bundle_data()).unwrap();

  let source_v1 = BundleSource::builder()
    .builtin_dir(&builtin_dir)
    .remote_dir(&remote_dir)
    .build();

  // The v1 descriptor a task holds after a cache hit.
  let v1_descriptor = source_v1.fetch_descriptor("app").await.unwrap();

  // The v2 file reader() would return after the version bumps but before the cache clears.
  let v2_reader = File::open(&v2_path).await.unwrap();

  let result = v1_descriptor.async_get_data(v2_reader, "/index.html").await;
  assert!(
    result.is_err(),
    "using a v1 descriptor to read a v2 file must produce an error, not silently return wrong data"
  );
}

// A manifest entry whose .wvb file is gone must yield BundleNotFound, not a panic.
#[tokio::test]
async fn missing_file_returns_not_found() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/index.html", BundleEntry::new(b"hello", "text/html", None)),
    )
    .set_builtin_current_version("app", "1.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();

  // Delete the .wvb file but leave the manifest intact.
  let wvb_path = builtin_dir.join("app").join("app_1.0.0.wvb");
  std::fs::remove_file(&wvb_path).unwrap();

  let source = Arc::new(
    BundleSource::builder()
      .builtin_dir(&builtin_dir)
      .remote_dir(&remote_dir)
      .build(),
  );
  let protocol = BundleProtocol::new(source.clone());

  let err = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap_err();
  assert!(
    matches!(err, wvb::Error::BundleNotFound),
    "expected BundleNotFound, got: {err}"
  );
}

// Invalid manifest JSON (e.g. truncated by a crash) must surface an error, not parse silently.
#[tokio::test]
async fn corrupt_manifest_errors() {
  let temp = TempDir::new();
  let builtin_dir = temp.dir().join("builtin");
  let remote_dir = temp.dir().join("remote");
  std::fs::create_dir_all(&builtin_dir).unwrap();
  std::fs::create_dir_all(&remote_dir).unwrap();

  std::fs::write(
    builtin_dir.join("manifest.json"),
    b"{ this is not valid json ",
  )
  .unwrap();

  let source = BundleSource::builder()
    .builtin_dir(&builtin_dir)
    .remote_dir(&remote_dir)
    .build();

  let result = source.load_version("app").await;
  assert!(
    result.is_err(),
    "corrupted manifest must return an error, not silently produce None"
  );
}

// A .wvb file of random bytes (e.g. a partial write) must fail to parse, not 200 with garbage.
#[tokio::test]
async fn corrupt_bundle_errors() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>hello</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();

  let wvb_path = builtin_dir.join("app").join("app_1.0.0.wvb");
  std::fs::write(&wvb_path, b"this is not a valid wvb file at all !!!").unwrap();

  let source = Arc::new(
    BundleSource::builder()
      .builtin_dir(&builtin_dir)
      .remote_dir(&remote_dir)
      .build(),
  );
  let protocol = BundleProtocol::new(source.clone());

  let result = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await;
  assert!(
    result.is_err(),
    "corrupted bundle file must return an error, not a 200 with garbage"
  );
}

// A zero-byte .wvb file must be rejected.
#[tokio::test]
async fn empty_bundle_errors() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/index.html", BundleEntry::new(b"hello", "text/html", None)),
    )
    .set_builtin_current_version("app", "1.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();
  std::fs::write(builtin_dir.join("app").join("app_1.0.0.wvb"), b"").unwrap();

  let source = Arc::new(
    BundleSource::builder()
      .builtin_dir(&builtin_dir)
      .remote_dir(&remote_dir)
      .build(),
  );
  let protocol = BundleProtocol::new(source.clone());

  let result = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await;
  assert!(result.is_err(), "empty .wvb file must return an error");
}

// A truncated .wvb file (interrupted download / power loss) must be rejected without panic.
#[tokio::test]
async fn truncated_bundle_errors() {
  let mut system = MockSystem::new();
  let bundle = MockBundle::new("app", "1.0.0").with_entry(
    "/index.html",
    BundleEntry::new(b"<h1>content</h1>", "text/html", None),
  );
  system
    .source_mut()
    .add_builtin_bundle(bundle.clone())
    .set_builtin_current_version("app", "1.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();

  let valid_bytes = bundle.bundle_data();
  let wvb_path = builtin_dir.join("app").join("app_1.0.0.wvb");
  std::fs::write(&wvb_path, &valid_bytes[..10.min(valid_bytes.len())]).unwrap();

  let source = Arc::new(
    BundleSource::builder()
      .builtin_dir(&builtin_dir)
      .remote_dir(&remote_dir)
      .build(),
  );
  let protocol = BundleProtocol::new(source.clone());

  let result = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await;
  assert!(result.is_err(), "truncated .wvb file must return an error");
}

// A .wvb file with no manifest entry (crash before save) must be invisible to load_version.
#[tokio::test]
async fn orphan_bundle_not_visible() {
  let temp = TempDir::new();
  let builtin_dir = temp.dir().join("builtin");
  let remote_dir = temp.dir().join("remote").join("app");
  std::fs::create_dir_all(&builtin_dir).unwrap();
  std::fs::create_dir_all(&remote_dir).unwrap();

  // Drop a .wvb file directly, with no manifest entry.
  let bundle = MockBundle::new("app", "2.0.0").with_entry(
    "/index.html",
    BundleEntry::new(b"orphan", "text/html", None),
  );
  std::fs::write(remote_dir.join("app_2.0.0.wvb"), bundle.bundle_data()).unwrap();

  let source = BundleSource::builder()
    .builtin_dir(&builtin_dir)
    .remote_dir(temp.dir().join("remote"))
    .build();

  let version = source.load_version("app").await.unwrap();
  assert!(
    version.is_none(),
    "a .wvb file without a manifest entry must not be visible to load_version"
  );
}

// Bytes modified after creation must fail to parse (fixed magic number + internal checksums).
#[tokio::test]
async fn bit_flipped_bundle_errors() {
  let mut system = MockSystem::new();
  let bundle = MockBundle::new("app", "1.0.0").with_entry(
    "/index.html",
    BundleEntry::new(b"<h1>original</h1>", "text/html", None),
  );
  system
    .source_mut()
    .add_builtin_bundle(bundle.clone())
    .set_builtin_current_version("app", "1.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();

  let wvb_path = builtin_dir.join("app").join("app_1.0.0.wvb");
  let mut bytes = std::fs::read(&wvb_path).unwrap();
  let mid = bytes.len() / 2;
  bytes[mid] ^= 0xFF;
  std::fs::write(&wvb_path, &bytes).unwrap();

  let source = Arc::new(
    BundleSource::builder()
      .builtin_dir(&builtin_dir)
      .remote_dir(&remote_dir)
      .build(),
  );
  let protocol = BundleProtocol::new(source.clone());

  let result = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await;
  assert!(
    result.is_err(),
    "a bit-flipped bundle must be rejected, not silently served"
  );
}
