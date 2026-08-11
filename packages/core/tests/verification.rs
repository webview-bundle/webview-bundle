//! Runtime verification: per-entry checksums when data is read, and integrity when a bundle
//! is loaded from disk.

mod common;

use common::{
  INDEX, SourceDirs, bundle, corrupt, get, reload_with, remote_server, update_all, updater,
  updater_with,
};
use std::sync::Arc;
use wvb::integrity::{IntegrityAlgorithm, IntegrityPolicy};
use wvb::protocol::{BundleProtocol, Protocol};
use wvb::source::{
  BundleSource, BundleSourceIntegrityOptions, BundleSourceOptions, BundleSourceVerifyMode,
};
use wvb::testing::{TestingBundle, TestingSourceBuilder};
use wvb::updater::{UpdaterGetUpdateOptions, UpdaterIntegrityOptions, UpdaterOptions};
use wvb::{BundleBuilderOptions, ChecksumReadOptions, ChecksumWriteOptions, DataReadOptions};

const BODY: &[u8] = b"<h1>hello</h1>";

fn app(version: &str) -> TestingBundle {
  bundle("app", version, BODY)
}

enum Kind {
  Builtin,
  Remote,
}

/// A source serving `bundle` from `kind`, optionally with integrity metadata recorded for it.
fn source_of(
  bundle: TestingBundle,
  kind: Kind,
  integrity: bool,
  options: BundleSourceOptions,
) -> (Arc<BundleSource>, SourceDirs) {
  let (name, version) = (bundle.name().to_owned(), bundle.version().to_owned());
  let mut builder = TestingSourceBuilder::new();
  if integrity {
    builder.set_integrity_algorithm(IntegrityAlgorithm::Sha256);
  }
  match kind {
    Kind::Builtin => {
      builder.add_builtin_bundle(bundle);
    }
    Kind::Remote => {
      builder.add_remote_bundle(bundle);
      builder.set_remote_current_version(name.clone(), version.clone());
    }
  }
  let dirs = SourceDirs {
    builtin: builder.builtin_dir().to_path_buf(),
    remote: builder.remote_dir().to_path_buf(),
  };
  let source = Arc::new(builder.build_with_options(Some(options)).unwrap());
  (source, dirs)
}

fn builtin_source(options: BundleSourceOptions) -> (Arc<BundleSource>, SourceDirs) {
  source_of(app("1.0.0"), Kind::Builtin, false, options)
}

fn filepath(source: &BundleSource, kind: Kind) -> std::path::PathBuf {
  match kind {
    Kind::Builtin => source.get_builtin_bundle_filepath("app", "1.0.0").unwrap(),
    Kind::Remote => source.get_remote_bundle_filepath("app", "1.0.0").unwrap(),
  }
}

fn integrity_options(policy: IntegrityPolicy) -> BundleSourceOptions {
  BundleSourceOptions::default().integrity(BundleSourceIntegrityOptions::default().policy(policy))
}

// ---------------------------------------------------------------------------
// Goal 1: the protocol verifies each entry's checksum as it serves it.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn protocol_serves_a_healthy_bundle() {
  let (source, _) = builtin_source(BundleSourceOptions::default());

  let resp = BundleProtocol::new(source)
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();

  assert_eq!(resp.status(), 200);
  assert_eq!(resp.body().as_ref(), BODY);
}

/// A byte flipped in an entry's compressed payload must surface as `ChecksumMismatch`,
/// not as damaged bytes handed to the webview.
#[tokio::test]
async fn protocol_rejects_a_corrupted_entry() {
  let (source, dirs) = builtin_source(BundleSourceOptions::default());
  let offset = app("1.0.0").entry_data_offset(INDEX).unwrap();
  corrupt(&filepath(&source, Kind::Builtin), |data| {
    data[offset] ^= 0xff
  });

  let err = BundleProtocol::new(reload_with(&dirs, BundleSourceOptions::default()))
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap_err();

  assert!(matches!(err, wvb::Error::ChecksumMismatch));
  assert_eq!(err.code(), wvb::ErrorCode::ChecksumMismatch);
}

/// Corrupting the stored checksum itself is caught too.
#[tokio::test]
async fn protocol_rejects_a_corrupted_checksum() {
  let (source, dirs) = builtin_source(BundleSourceOptions::default());
  let offset = app("1.0.0").entry_checksum_offset(INDEX).unwrap();
  corrupt(&filepath(&source, Kind::Builtin), |data| {
    data[offset] ^= 0xff
  });

  let err = BundleProtocol::new(reload_with(&dirs, BundleSourceOptions::default()))
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap_err();

  assert!(matches!(err, wvb::Error::ChecksumMismatch));
}

/// Range requests read entry data through the same path, so they are verified too.
#[tokio::test]
async fn protocol_rejects_a_corrupted_entry_on_a_range_request() {
  let (source, dirs) = builtin_source(BundleSourceOptions::default());
  let offset = app("1.0.0").entry_data_offset(INDEX).unwrap();
  corrupt(&filepath(&source, Kind::Builtin), |data| {
    data[offset] ^= 0xff
  });

  let request = http::Request::builder()
    .uri("https://app.wvb/index.html")
    .method("GET")
    .header(http::header::RANGE, "bytes=0-4")
    .body(vec![])
    .unwrap();
  let err = BundleProtocol::new(reload_with(&dirs, BundleSourceOptions::default()))
    .handle(request)
    .await
    .unwrap_err();

  assert!(matches!(err, wvb::Error::ChecksumMismatch));
}

#[tokio::test]
async fn protocol_verification_can_be_turned_off() {
  let (source, dirs) = builtin_source(BundleSourceOptions::default());
  let offset = app("1.0.0").entry_checksum_offset(INDEX).unwrap();
  corrupt(&filepath(&source, Kind::Builtin), |data| {
    data[offset] ^= 0xff
  });

  // Only the checksum byte is corrupt, so the entry still decompresses once the source is
  // told not to verify it.
  let options = BundleSourceOptions::default()
    .data_read(DataReadOptions::default().checksum(ChecksumReadOptions::default().verify(false)));
  let resp = BundleProtocol::new(reload_with(&dirs, options))
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();

  assert_eq!(resp.body().as_ref(), BODY);
}

/// The protocol serves entries with the checksum seed the source is configured with: a bundle
/// packed with a non-zero seed is served only when the source carries the same seed.
#[tokio::test]
async fn protocol_serves_with_the_source_checksum_seed() {
  let mut bundle = app("1.0.0");
  bundle.set_options(
    BundleBuilderOptions::default().data_checksum(ChecksumWriteOptions::default().seed(42)),
  );
  let matching = BundleSourceOptions::default()
    .data_read(DataReadOptions::default().checksum(ChecksumReadOptions::default().seed(42)));
  let (source, dirs) = source_of(bundle, Kind::Builtin, false, matching);

  let resp = BundleProtocol::new(source)
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), BODY);

  // The default source seed (0) recomputes a different checksum for the very same bytes.
  let err = BundleProtocol::new(reload_with(&dirs, BundleSourceOptions::default()))
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap_err();
  assert!(matches!(err, wvb::Error::ChecksumMismatch));
}

/// `LoadedDescriptor::get_data` is what the bindings read through directly, without going via
/// the protocol, so it verifies by default too.
#[tokio::test]
async fn loaded_descriptor_verifies_by_default() {
  let (source, dirs) = builtin_source(BundleSourceOptions::default());
  let offset = app("1.0.0").entry_data_offset(INDEX).unwrap();
  corrupt(&filepath(&source, Kind::Builtin), |data| {
    data[offset] ^= 0xff
  });

  let source = reload_with(&dirs, BundleSourceOptions::default());
  let descriptor = source.load("app").await.unwrap();
  let err = descriptor.get_data(INDEX).await.unwrap_err();

  assert!(matches!(err, wvb::Error::ChecksumMismatch));
}

/// Reading a descriptor verifies the header checksum, so a damaged header is caught on load
/// even without integrity metadata. The header checksum sits at a fixed 4-byte offset;
/// flipping a byte of it leaves the header fields intact but breaks the checksum.
#[tokio::test]
async fn load_verifies_the_header_checksum() {
  let (source, dirs) = builtin_source(BundleSourceOptions::default());
  corrupt(&filepath(&source, Kind::Builtin), |data| data[13] ^= 0xff);

  let err = reload_with(&dirs, BundleSourceOptions::default())
    .load("app")
    .await
    .unwrap_err();

  assert!(matches!(err, wvb::Error::InvalidHeaderChecksum));
}

/// Reading a descriptor verifies the index checksum too. The index checksum follows the
/// index content, which begins after the 17-byte header; flipping its first byte breaks it.
#[tokio::test]
async fn load_verifies_the_index_checksum() {
  let (source, dirs) = builtin_source(BundleSourceOptions::default());
  corrupt(&filepath(&source, Kind::Builtin), |data| {
    let index_size = u32::from_be_bytes([data[9], data[10], data[11], data[12]]) as usize;
    data[17 + index_size] ^= 0xff;
  });

  let err = reload_with(&dirs, BundleSourceOptions::default())
    .load("app")
    .await
    .unwrap_err();

  assert!(matches!(err, wvb::Error::InvalidIndexChecksum));
}

/// Header/index verification can be turned off through the source's read options.
#[tokio::test]
async fn header_index_verification_can_be_turned_off() {
  use wvb::{HeaderReadOptions, IndexReadOptions};

  let (source, dirs) = builtin_source(BundleSourceOptions::default());
  corrupt(&filepath(&source, Kind::Builtin), |data| data[13] ^= 0xff);

  let options = BundleSourceOptions::default()
    .header_read(
      HeaderReadOptions::default().checksum(ChecksumReadOptions::default().verify(false)),
    )
    .index_read(IndexReadOptions::default().checksum(ChecksumReadOptions::default().verify(false)));

  reload_with(&dirs, options).load("app").await.unwrap();
}

// ---------------------------------------------------------------------------
// Goal 2: the source verifies integrity when it loads a bundle.
// ---------------------------------------------------------------------------

/// Flipping a byte of the data section leaves the header and index intact, so the bundle
/// still parses — only the whole-file hash catches it.
#[tokio::test]
async fn load_detects_a_corrupted_remote_bundle() {
  let options = integrity_options(IntegrityPolicy::Strict);
  let (source, dirs) = source_of(app("1.0.0"), Kind::Remote, true, options.clone());
  let offset = app("1.0.0").entry_data_offset(INDEX).unwrap();
  corrupt(&filepath(&source, Kind::Remote), |data| {
    data[offset] ^= 0xff
  });

  let err = reload_with(&dirs, options).load("app").await.unwrap_err();

  assert!(matches!(err, wvb::Error::IntegrityVerifyFailed));
}

#[tokio::test]
async fn load_accepts_an_intact_remote_bundle() {
  let options = integrity_options(IntegrityPolicy::Strict);
  let (source, _) = source_of(app("1.0.0"), Kind::Remote, true, options);

  let resp = BundleProtocol::new(source)
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();

  assert_eq!(resp.body().as_ref(), BODY);
}

/// `BundleSourceVerifyMode::OnlyRemote` must not require integrity metadata on builtin
/// bundles, which is the whole reason the mode exists: builtin manifests carry no integrity
/// unless the app was packed with it.
#[tokio::test]
async fn check_mode_only_remote_leaves_builtin_bundles_alone() {
  let (source, _) = builtin_source(integrity_options(IntegrityPolicy::Strict));

  let resp = BundleProtocol::new(source)
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();

  assert_eq!(resp.body().as_ref(), BODY);
}

/// ...whereas `All` does verify them, so a missing integrity string fails under `Strict`.
#[tokio::test]
async fn check_mode_all_with_strict_policy_requires_builtin_integrity() {
  let options = BundleSourceOptions::default().integrity(
    BundleSourceIntegrityOptions::default()
      .policy(IntegrityPolicy::Strict)
      .check_mode(BundleSourceVerifyMode::All),
  );
  let (source, _) = builtin_source(options);

  let err = source.load("app").await.unwrap_err();

  assert!(matches!(err, wvb::Error::IntegrityVerifyFailed));
}

/// `All` selects *which* bundles are hashed; the policy decides whether a bundle with no
/// integrity string at all is an error. Under the default `Optional` policy it is not.
#[tokio::test]
async fn check_mode_all_under_optional_policy_allows_missing_integrity() {
  let options = BundleSourceOptions::default()
    .integrity(BundleSourceIntegrityOptions::default().check_mode(BundleSourceVerifyMode::All));
  let (source, _) = builtin_source(options);

  let resp = BundleProtocol::new(source)
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();

  assert_eq!(resp.body().as_ref(), BODY);
}

/// By default the integrity check applies to remote bundles under the optional policy, so a
/// remote bundle whose manifest carries integrity metadata is hashed on load.
#[tokio::test]
async fn integrity_check_defaults_to_only_remote_under_the_optional_policy() {
  let (source, dirs) = source_of(
    app("1.0.0"),
    Kind::Remote,
    true,
    BundleSourceOptions::default(),
  );
  let offset = app("1.0.0").entry_data_offset(INDEX).unwrap();
  corrupt(&filepath(&source, Kind::Remote), |data| {
    data[offset] ^= 0xff
  });

  let err = reload_with(&dirs, BundleSourceOptions::default())
    .load("app")
    .await
    .unwrap_err();

  assert!(matches!(err, wvb::Error::IntegrityVerifyFailed));
}

/// Under the default policy a remote bundle without integrity metadata is not hashed on load.
/// Corruption is still caught on read, by the entry checksum — the two layers are
/// complementary.
#[tokio::test]
async fn a_remote_bundle_without_integrity_metadata_loads_by_default() {
  let (source, dirs) = source_of(
    app("1.0.0"),
    Kind::Remote,
    false,
    BundleSourceOptions::default(),
  );
  let offset = app("1.0.0").entry_data_offset(INDEX).unwrap();
  corrupt(&filepath(&source, Kind::Remote), |data| {
    data[offset] ^= 0xff
  });

  let source = reload_with(&dirs, BundleSourceOptions::default());
  source.load("app").await.unwrap();

  let err = BundleProtocol::new(source)
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap_err();
  assert!(matches!(err, wvb::Error::ChecksumMismatch));
}

/// The updater and the source must agree: a bundle the updater accepted on install is the
/// bundle the source verifies on load, byte for byte.
#[tokio::test]
async fn a_downloaded_bundle_verifies_on_load() {
  let options = integrity_options(IntegrityPolicy::Strict);
  let (source, _) = source_of(app("1.0.0"), Kind::Builtin, false, options);
  let server = remote_server(vec![bundle("app", "2.0.0", b"<h1>v2</h1>")]);
  let updater = updater_with(
    &source,
    &server.base_url(),
    UpdaterOptions::default()
      .integrity(UpdaterIntegrityOptions::default().policy(IntegrityPolicy::Strict)),
  );

  update_all(&updater).await;

  let resp = BundleProtocol::new(source)
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>v2</h1>");
}

/// An update whose integrity does not describe the bytes that were served must not be
/// installed, whatever the server claims.
#[tokio::test]
async fn install_rejects_a_bundle_the_integrity_does_not_describe() {
  let options = integrity_options(IntegrityPolicy::Strict);
  let (source, _) = source_of(app("1.0.0"), Kind::Builtin, false, options);
  let server = remote_server(vec![bundle("app", "2.0.0", b"<h1>v2</h1>")]);
  let updater = updater(&source, &server.base_url());

  let update = common::fetch_update(&updater).await;
  common::download_all(&updater, &update.bundles).await;
  // The downloaded file is swapped for other bytes after the server vouched for it.
  corrupt(
    &source.get_remote_bundle_filepath("app", "2.0.0").unwrap(),
    |data| {
      let offset = bundle("app", "2.0.0", b"<h1>v2</h1>")
        .entry_data_offset(INDEX)
        .unwrap();
      data[offset] ^= 0xff;
    },
  );

  let results = updater
    .install(&[common::target("app", "2.0.0")])
    .await
    .unwrap();

  assert_eq!(
    results[0].result.as_ref().unwrap_err().code(),
    wvb::ErrorCode::IntegrityVerifyFailed
  );
}

/// The update itself is authenticated by a signature over the response body: one signed by a
/// key the client does not hold must be rejected before any bundle is downloaded.
#[cfg(feature = "signature-ed25519")]
#[tokio::test]
async fn an_update_signed_by_an_unexpected_key_is_rejected() {
  use ed25519_dalek::SigningKey;
  use wvb::signature::{Ed25519, SignatureKey, SignatureKeySet};
  use wvb::updater::UpdaterSignatureOptions;

  let (source, _) = builtin_source(BundleSourceOptions::default());
  let mut server = remote_server(vec![bundle("app", "2.0.0", b"<h1>v2</h1>")]);
  server.insert_signature_key("release", [7u8; 32]);

  // A key published under the same id, but not the pair the server signs with.
  let other = SigningKey::from_bytes(&[9u8; 32]);
  let key_set = SignatureKeySet {
    id: "release".to_owned(),
    key: SignatureKey::Ed25519(
      Ed25519::from_public_key_bytes(&other.verifying_key().to_bytes()).unwrap(),
    ),
  };
  let updater = updater_with(
    &source,
    &server.base_url(),
    UpdaterOptions::default().signature(UpdaterSignatureOptions::default().key_set(key_set)),
  );

  let err = updater
    .get_update(Some(
      UpdaterGetUpdateOptions::default().expect_signature_key_id("release"),
    ))
    .await
    .unwrap_err();

  assert_eq!(err.code(), wvb::ErrorCode::SignatureVerifyFailed);
}
