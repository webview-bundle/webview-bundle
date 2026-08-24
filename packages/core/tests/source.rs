mod common;

use common::{bundle, corrupt, get, reload, remote_server, source_with_dirs, update_all, updater};
use std::path::PathBuf;
use wvb::protocol::{BundleProtocol, Protocol};
use wvb::source::Source;
use wvb::testing::TempDir;

fn builtin_filepath(source: &Source, version: &str) -> PathBuf {
  source.get_builtin_bundle_filepath("app", version).unwrap()
}

#[tokio::test]
async fn manifest_persists_across_reload() {
  let (source, dirs) = source_with_dirs(vec![bundle("app", "1.0.0", b"<h1>builtin</h1>")]);
  let server = remote_server(vec![bundle("app", "2.0.0", b"<h1>remote</h1>")]);

  update_all(&updater(&source, &server.base_url())).await;

  let reloaded = reload(&dirs);
  let version = reloaded.get_version("app").await.unwrap().unwrap();
  assert_eq!(
    version.version, "2.0.0",
    "downloaded version must survive a source reload"
  );

  let protocol = BundleProtocol::new(reloaded);
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
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

  let (source, _dirs) = source_with_dirs(vec![bundle("app", "1.0.0", V1)]);
  let server = remote_server(vec![bundle("app", "2.0.0", V2)]);
  let protocol = BundleProtocol::new(source.clone());

  // Warm the descriptor cache with v1.
  let warm = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(warm.body().as_ref(), V1);

  // Activation changes the active filepath; a stale v1 descriptor would mis-read the v2 file.
  update_all(&updater(&source, &server.base_url())).await;

  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
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
  const V1: &[u8] = b"<h1>v1</h1>";
  const V2: &[u8] = b"<h1>version 2 - longer content to guarantee different LZ4 size</h1>";

  let (source, dirs) = source_with_dirs(vec![bundle("app", "1.0.0", V1)]);

  // Write the v2 file directly so the manifest still points at v1 (present but inactive).
  let v2_path = dirs.remote.join("app").join("2.0.0.wvb");
  std::fs::create_dir_all(v2_path.parent().unwrap()).unwrap();
  std::fs::write(
    &v2_path,
    bundle("app", "2.0.0", V2).make_bundle_data().unwrap(),
  )
  .unwrap();

  // The v1 descriptor a task holds after a cache hit.
  let v1_descriptor = source.fetch_descriptor("app").await.unwrap();
  // The v2 file reader() would return after the version bumps but before the cache clears.
  let v2_reader = tokio::fs::File::open(&v2_path).await.unwrap();

  let result = v1_descriptor.async_get_data(v2_reader, "/index.html").await;

  assert!(
    result.is_err(),
    "using a v1 descriptor to read a v2 file must produce an error, not silently return wrong data"
  );
}

// A manifest entry whose .wvb file is gone must yield BundleNotFound, not a panic.
#[tokio::test]
async fn missing_file_returns_not_found() {
  let (source, dirs) = source_with_dirs(vec![bundle("app", "1.0.0", b"hello")]);
  std::fs::remove_file(builtin_filepath(&source, "1.0.0")).unwrap();

  let protocol = BundleProtocol::new(reload(&dirs));
  let err = protocol
    .handle(get("https://app.wvb/index.html"))
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
  std::fs::create_dir_all(&builtin_dir).unwrap();
  std::fs::write(
    builtin_dir.join("manifest.json"),
    b"{ this is not valid json ",
  )
  .unwrap();

  let source = Source::builder()
    .builtin_dir(&builtin_dir)
    .remote_dir(temp.dir().join("remote"))
    .build();

  assert!(
    source.get_version("app").await.is_err(),
    "corrupted manifest must return an error, not silently produce None"
  );
}

// A .wvb file of random bytes (e.g. a partial write) must fail to parse, not 200 with garbage.
#[tokio::test]
async fn corrupt_bundle_errors() {
  let (source, dirs) = source_with_dirs(vec![bundle("app", "1.0.0", b"<h1>hello</h1>")]);
  std::fs::write(
    builtin_filepath(&source, "1.0.0"),
    b"this is not a valid wvb file at all !!!",
  )
  .unwrap();

  let protocol = BundleProtocol::new(reload(&dirs));

  assert!(
    protocol
      .handle(get("https://app.wvb/index.html"))
      .await
      .is_err(),
    "corrupted bundle file must return an error, not a 200 with garbage"
  );
}

// A zero-byte .wvb file must be rejected.
#[tokio::test]
async fn empty_bundle_errors() {
  let (source, dirs) = source_with_dirs(vec![bundle("app", "1.0.0", b"hello")]);
  std::fs::write(builtin_filepath(&source, "1.0.0"), b"").unwrap();

  let protocol = BundleProtocol::new(reload(&dirs));

  assert!(
    protocol
      .handle(get("https://app.wvb/index.html"))
      .await
      .is_err(),
    "empty .wvb file must return an error"
  );
}

// A truncated .wvb file (interrupted download / power loss) must be rejected without panic.
#[tokio::test]
async fn truncated_bundle_errors() {
  let (source, dirs) = source_with_dirs(vec![bundle("app", "1.0.0", b"<h1>content</h1>")]);
  let filepath = builtin_filepath(&source, "1.0.0");
  let data = std::fs::read(&filepath).unwrap();
  std::fs::write(&filepath, &data[..10.min(data.len())]).unwrap();

  let protocol = BundleProtocol::new(reload(&dirs));

  assert!(
    protocol
      .handle(get("https://app.wvb/index.html"))
      .await
      .is_err(),
    "truncated .wvb file must return an error"
  );
}

// A .wvb file with no manifest entry (crash before save) must be invisible to the source.
#[tokio::test]
async fn orphan_bundle_not_visible() {
  let temp = TempDir::new();
  let remote_dir = temp.dir().join("remote");
  std::fs::create_dir_all(remote_dir.join("app")).unwrap();

  // Drop a .wvb file directly, with no manifest entry.
  std::fs::write(
    remote_dir.join("app").join("2.0.0.wvb"),
    bundle("app", "2.0.0", b"orphan")
      .make_bundle_data()
      .unwrap(),
  )
  .unwrap();

  let source = Source::builder()
    .builtin_dir(temp.dir().join("builtin"))
    .remote_dir(&remote_dir)
    .build();

  assert!(
    source.get_version("app").await.unwrap().is_none(),
    "a .wvb file without a manifest entry must not be visible to the source"
  );
}

// Bytes modified after creation must fail to parse (fixed magic number + internal checksums).
#[tokio::test]
async fn bit_flipped_bundle_errors() {
  let (source, dirs) = source_with_dirs(vec![bundle("app", "1.0.0", b"<h1>original</h1>")]);
  corrupt(&builtin_filepath(&source, "1.0.0"), |data| {
    let mid = data.len() / 2;
    data[mid] ^= 0xff;
  });

  let protocol = BundleProtocol::new(reload(&dirs));

  assert!(
    protocol
      .handle(get("https://app.wvb/index.html"))
      .await
      .is_err(),
    "a bit-flipped bundle must be rejected, not silently served"
  );
}
