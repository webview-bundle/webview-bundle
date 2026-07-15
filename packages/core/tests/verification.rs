//! Runtime verification: per-entry checksums when data is read, and integrity/signature
//! when a bundle is loaded from disk.

mod common;

use base64ct::{Base64, Encoding};
use common::get;
use ed25519_dalek::{Signer, SigningKey};
use std::sync::Arc;
use wvb::integrity::IntegrityPolicy;
use wvb::protocol::{BundleProtocol, BundleProtocolOptions, Protocol};
use wvb::signature::{Ed25519Verifier, SignatureVerifier};
use wvb::source::{
  BundleSourceIntegrityCheckMode, BundleSourceIntegrityOptions, BundleSourceOptions,
  BundleSourceSignatureOptions, BundleSourceSignatureVerifyMode,
};
use wvb::testing::*;
use wvb::updater::{Updater, UpdaterConfig};
use wvb::{BundleBuilderOptions, BundleEntry, DataReadChecksumOptions, DataReadOptions};

const INDEX: &str = "/index.html";
const BODY: &[u8] = b"<h1>hello</h1>";

fn app(version: &str) -> MockBundle {
  MockBundle::new("app", version).with_entry(INDEX, BundleEntry::new(BODY, "text/html", None))
}

fn signing_key() -> SigningKey {
  SigningKey::from_bytes(&[7u8; 32])
}

fn verifier() -> SignatureVerifier {
  let key = Ed25519Verifier::from_public_key_bytes(&signing_key().verifying_key().to_bytes())
    .expect("valid verifying key");
  SignatureVerifier::Ed25519(Arc::new(key))
}

fn sign(message: &[u8]) -> String {
  Base64::encode_string(&signing_key().sign(message).to_bytes())
}

// ---------------------------------------------------------------------------
// Goal 1: the protocol verifies each entry's checksum as it serves it.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn protocol_serves_a_healthy_bundle() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0"))
    .set_builtin_current_version("app", "1.0.0");

  let protocol = BundleProtocol::new(Arc::new(system.source().get_source()));
  let resp = protocol
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
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0"))
    .set_builtin_current_version("app", "1.0.0");

  let data = app("1.0.0").bundle_data();
  let offset = entry_data_offset(&data, INDEX);
  system
    .source()
    .corrupt_builtin_bundle("app", "1.0.0", |data| data[offset] ^= 0xff);

  let protocol = BundleProtocol::new(Arc::new(system.source().get_source()));
  let err = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap_err();
  assert!(matches!(err, wvb::Error::ChecksumMismatch));
  assert_eq!(err.code(), wvb::ErrorCode::ChecksumMismatch);
}

/// Corrupting the stored checksum itself is caught too.
#[tokio::test]
async fn protocol_rejects_a_corrupted_checksum() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0"))
    .set_builtin_current_version("app", "1.0.0");

  let data = app("1.0.0").bundle_data();
  let offset = entry_checksum_offset(&data, INDEX);
  system
    .source()
    .corrupt_builtin_bundle("app", "1.0.0", |data| data[offset] ^= 0xff);

  let protocol = BundleProtocol::new(Arc::new(system.source().get_source()));
  let err = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap_err();
  assert!(matches!(err, wvb::Error::ChecksumMismatch));
}

/// Range requests read entry data through the same path, so they are verified too.
#[tokio::test]
async fn protocol_rejects_a_corrupted_entry_on_a_range_request() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0"))
    .set_builtin_current_version("app", "1.0.0");

  let data = app("1.0.0").bundle_data();
  let offset = entry_data_offset(&data, INDEX);
  system
    .source()
    .corrupt_builtin_bundle("app", "1.0.0", |data| data[offset] ^= 0xff);

  let protocol = BundleProtocol::new(Arc::new(system.source().get_source()));
  let request = http::Request::builder()
    .uri("https://app.wvb/index.html")
    .method("GET")
    .header(http::header::RANGE, "bytes=0-4")
    .body(vec![])
    .unwrap();
  let err = protocol.handle(request).await.unwrap_err();
  assert!(matches!(err, wvb::Error::ChecksumMismatch));
}

#[tokio::test]
async fn protocol_verification_can_be_turned_off() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0"))
    .set_builtin_current_version("app", "1.0.0");

  let data = app("1.0.0").bundle_data();
  let offset = entry_checksum_offset(&data, INDEX);
  system
    .source()
    .corrupt_builtin_bundle("app", "1.0.0", |data| data[offset] ^= 0xff);

  // Only the checksum is damaged, so with verification off the payload still decompresses.
  let protocol = BundleProtocol::new(Arc::new(system.source().get_source()))
    .with_options(BundleProtocolOptions::default().verify_data_checksum(false));
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), BODY);
}

/// A bundle packed with a non-zero seed must be served with the same seed.
#[tokio::test]
async fn protocol_honours_the_checksum_seed() {
  let mut options = BundleBuilderOptions::default();
  options.data_checksum_seed(42);

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0").with_builder_options(options))
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());

  let matching = BundleProtocol::new(source.clone())
    .with_options(BundleProtocolOptions::default().data_checksum_seed(42));
  let resp = matching
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), BODY);

  // The default seed (0) recomputes a different checksum for the very same bytes.
  let mismatched = BundleProtocol::new(source);
  let err = mismatched
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap_err();
  assert!(matches!(err, wvb::Error::ChecksumMismatch));
}

/// `LoadedDescriptor::get_data` is what the bindings read through directly, without going via
/// the protocol, so it verifies by default too.
#[tokio::test]
async fn loaded_descriptor_verifies_by_default() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0"))
    .set_builtin_current_version("app", "1.0.0");

  let data = app("1.0.0").bundle_data();
  let offset = entry_data_offset(&data, INDEX);
  system
    .source()
    .corrupt_builtin_bundle("app", "1.0.0", |data| data[offset] ^= 0xff);

  let source = system.source().get_source();
  let descriptor = source.load_descriptor("app").await.unwrap();
  let err = descriptor.get_data(INDEX).await.unwrap_err();
  assert!(matches!(err, wvb::Error::ChecksumMismatch));
}

/// The source's own read options apply to `LoadedDescriptor::get_data`, which bindings use
/// directly, without going through the protocol.
#[tokio::test]
async fn source_read_options_apply_to_loaded_descriptor() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0"))
    .set_builtin_current_version("app", "1.0.0");

  let data = app("1.0.0").bundle_data();
  let offset = entry_data_offset(&data, INDEX);
  system
    .source()
    .corrupt_builtin_bundle("app", "1.0.0", |data| data[offset] ^= 0xff);

  let options = BundleSourceOptions::default().data_read_options(
    DataReadOptions::default().checksum(DataReadChecksumOptions::default().verify(true)),
  );
  let source = system.source().get_source_with(options);
  let descriptor = source.load_descriptor("app").await.unwrap();
  let err = descriptor.get_data(INDEX).await.unwrap_err();
  assert!(matches!(err, wvb::Error::ChecksumMismatch));
}

/// Reading a descriptor verifies the header checksum, so a damaged header is caught on load
/// even without integrity metadata. The header checksum sits at a fixed 4-byte offset;
/// flipping a byte of it leaves the header fields intact but breaks the checksum.
#[tokio::test]
async fn load_verifies_the_header_checksum() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0"))
    .set_builtin_current_version("app", "1.0.0");
  system
    .source()
    .corrupt_builtin_bundle("app", "1.0.0", |data| data[13] ^= 0xff);

  let source = system.source().get_source();
  let err = source.load_descriptor("app").await.unwrap_err();
  assert!(matches!(err, wvb::Error::InvalidHeaderChecksum));
}

/// Reading a descriptor verifies the index checksum too. The index checksum follows the
/// index content, which begins after the 17-byte header; flipping its first byte breaks it.
#[tokio::test]
async fn load_verifies_the_index_checksum() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0"))
    .set_builtin_current_version("app", "1.0.0");
  system
    .source()
    .corrupt_builtin_bundle("app", "1.0.0", |data| {
      let index_size = u32::from_be_bytes([data[9], data[10], data[11], data[12]]) as usize;
      data[17 + index_size] ^= 0xff;
    });

  let source = system.source().get_source();
  let err = source.load_descriptor("app").await.unwrap_err();
  assert!(matches!(err, wvb::Error::InvalidIndexChecksum));
}

/// Header/index verification can be turned off through the source's read options.
#[tokio::test]
async fn header_index_verification_can_be_turned_off() {
  use wvb::{
    HeaderReadChecksumOptions, HeaderReadOptions, IndexReadChecksumOptions, IndexReadOptions,
  };

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0"))
    .set_builtin_current_version("app", "1.0.0");
  system
    .source()
    .corrupt_builtin_bundle("app", "1.0.0", |data| data[13] ^= 0xff);

  let options = BundleSourceOptions::default()
    .header_read_options(
      HeaderReadOptions::default().checksum(HeaderReadChecksumOptions::default().verify(false)),
    )
    .index_read_options(
      IndexReadOptions::default().checksum(IndexReadChecksumOptions::default().verify(false)),
    );
  let source = system.source().get_source_with(options);
  source.load_descriptor("app").await.unwrap();
}

// ---------------------------------------------------------------------------
// Goal 2: the source verifies integrity/signature when it loads a bundle.
// ---------------------------------------------------------------------------

/// Truncating a byte of the data section leaves the header and index intact, so the bundle
/// still parses — only the whole-file hash catches it.
#[tokio::test]
async fn load_detects_a_corrupted_remote_bundle() {
  let bundle = app("1.0.0").with_auto_integrity();

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_remote_bundle(bundle.clone())
    .set_remote_current_version("app", "1.0.0");

  let data = bundle.bundle_data();
  let offset = entry_data_offset(&data, INDEX);
  system
    .source()
    .corrupt_remote_bundle("app", "1.0.0", |data| data[offset] ^= 0xff);

  let options = BundleSourceOptions::default()
    .integrity(BundleSourceIntegrityOptions::default().policy(IntegrityPolicy::Strict));
  let source = system.source().get_source_with(options);

  let err = source.load_descriptor("app").await.unwrap_err();
  assert!(matches!(err, wvb::Error::IntegrityVerifyFailed));
}

#[tokio::test]
async fn load_accepts_an_intact_remote_bundle() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_remote_bundle(app("1.0.0").with_auto_integrity())
    .set_remote_current_version("app", "1.0.0");

  let options = BundleSourceOptions::default()
    .integrity(BundleSourceIntegrityOptions::default().policy(IntegrityPolicy::Strict));
  let source = Arc::new(system.source().get_source_with(options));

  let protocol = BundleProtocol::new(source);
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), BODY);
}

/// `BundleSourceIntegrityCheckMode::OnlyRemote` must not require integrity metadata on builtin
/// bundles, which is the whole reason the mode exists: builtin manifests carry no
/// integrity unless the app was packed with it.
#[tokio::test]
async fn check_mode_only_remote_leaves_builtin_bundles_alone() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0")) // no integrity metadata
    .set_builtin_current_version("app", "1.0.0");

  let options = BundleSourceOptions::default().integrity(
    BundleSourceIntegrityOptions::default()
      .policy(IntegrityPolicy::Strict)
      .check_mode(BundleSourceIntegrityCheckMode::OnlyRemote),
  );
  let source = Arc::new(system.source().get_source_with(options));

  let protocol = BundleProtocol::new(source);
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), BODY);
}

/// ...whereas `All` does verify them, so a missing integrity string fails under `Strict`.
///
/// It is the policy, not the `All` mode, that decides whether *missing* metadata is an
/// error — see `check_mode_all_under_optional_policy_allows_missing_integrity`.
#[tokio::test]
async fn check_mode_all_with_strict_policy_requires_builtin_integrity() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0"))
    .set_builtin_current_version("app", "1.0.0");

  let options = BundleSourceOptions::default().integrity(
    BundleSourceIntegrityOptions::default()
      .policy(IntegrityPolicy::Strict)
      .check_mode(BundleSourceIntegrityCheckMode::All),
  );
  let source = system.source().get_source_with(options);

  let err = source.load_descriptor("app").await.unwrap_err();
  assert!(matches!(err, wvb::Error::IntegrityVerifyFailed));
}

/// `All` selects *which* bundles are hashed; [`IntegrityPolicy`] decides whether a bundle
/// with no integrity string at all is an error. Under the default `Optional` policy it is
/// not, so an unhashed builtin bundle still loads.
#[tokio::test]
async fn check_mode_all_under_optional_policy_allows_missing_integrity() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0")) // no integrity metadata
    .set_builtin_current_version("app", "1.0.0");

  let options = BundleSourceOptions::default().integrity(
    BundleSourceIntegrityOptions::default().check_mode(BundleSourceIntegrityCheckMode::All),
  );
  let source = Arc::new(system.source().get_source_with(options));

  let protocol = BundleProtocol::new(source);
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), BODY);
}

/// By default the integrity check applies to remote bundles under the optional policy, so
/// a remote bundle whose manifest carries integrity metadata is hashed on load.
#[tokio::test]
async fn integrity_check_defaults_to_only_remote_under_the_optional_policy() {
  let bundle = app("1.0.0").with_auto_integrity();

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_remote_bundle(bundle.clone())
    .set_remote_current_version("app", "1.0.0");

  let data = bundle.bundle_data();
  let offset = entry_data_offset(&data, INDEX);
  system
    .source()
    .corrupt_remote_bundle("app", "1.0.0", |data| data[offset] ^= 0xff);

  let source = system.source().get_source();
  let err = source.load_descriptor("app").await.unwrap_err();
  assert!(matches!(err, wvb::Error::IntegrityVerifyFailed));
}

/// Under the default (optional) policy a remote bundle without integrity metadata is not
/// hashed on load. Corruption is still caught on read, by the entry checksum — the two
/// layers are complementary.
#[tokio::test]
async fn a_remote_bundle_without_integrity_metadata_loads_by_default() {
  let bundle = app("1.0.0"); // no integrity metadata

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_remote_bundle(bundle.clone())
    .set_remote_current_version("app", "1.0.0");

  let data = bundle.bundle_data();
  let offset = entry_data_offset(&data, INDEX);
  system
    .source()
    .corrupt_remote_bundle("app", "1.0.0", |data| data[offset] ^= 0xff);

  let source = Arc::new(system.source().get_source());
  source.load_descriptor("app").await.unwrap();

  let protocol = BundleProtocol::new(source);
  let err = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap_err();
  assert!(matches!(err, wvb::Error::ChecksumMismatch));
}

#[tokio::test]
async fn load_verifies_the_signature() {
  let bundle = app("1.0.0")
    .with_auto_integrity()
    .with_signed_integrity(sign);

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_remote_bundle(bundle)
    .set_remote_current_version("app", "1.0.0");

  let options = BundleSourceOptions::default()
    .signature(BundleSourceSignatureOptions::default().verify(verifier()));
  let source = Arc::new(system.source().get_source_with(options));

  let protocol = BundleProtocol::new(source);
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), BODY);
}

/// An attacker who swaps the bundle file has to swap its integrity string too, and cannot
/// sign the new one without the private key.
#[tokio::test]
async fn load_rejects_a_tampered_bundle_whose_integrity_was_updated() {
  let tampered = MockBundle::new("app", "1.0.0")
    .with_entry(INDEX, BundleEntry::new(b"<h1>evil</h1>", "text/html", None))
    .with_auto_integrity() // integrity recomputed over the tampered bytes...
    .with_signature(sign(b"sha256:the-signature-of-the-original")); // ...but not re-signed

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_remote_bundle(tampered)
    .set_remote_current_version("app", "1.0.0");

  let options = BundleSourceOptions::default()
    .signature(BundleSourceSignatureOptions::default().verify(verifier()));
  let source = system.source().get_source_with(options);

  let err = source.load_descriptor("app").await.unwrap_err();
  assert!(matches!(err, wvb::Error::SignatureVerifyFailed));
}

/// Integrity and signature verification run independently. The signature authenticates the
/// integrity *string*, not the bytes it describes: with the integrity policy off, a validly
/// signed bundle loads even when the advertised hash does not match its bytes. Keeping the
/// integrity check enabled is what closes that gap.
#[tokio::test]
async fn signature_verification_runs_independently_of_the_integrity_check() {
  let integrity_of_other_bytes = MockBundle::new("app", "1.0.0")
    .with_entry(INDEX, BundleEntry::new(b"other", "text/html", None))
    .with_auto_integrity();
  let advertised = integrity_of_other_bytes.integrity().unwrap().to_string();

  let bundle = app("1.0.0")
    .with_integrity(advertised.clone())
    .with_signature(sign(advertised.as_bytes()));

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_remote_bundle(bundle)
    .set_remote_current_version("app", "1.0.0");

  // Integrity off: only the signature is verified, and it is valid over the advertised
  // string — the mismatched hash goes unnoticed.
  let options = BundleSourceOptions::default()
    .integrity(BundleSourceIntegrityOptions::default().policy(IntegrityPolicy::Off))
    .signature(BundleSourceSignatureOptions::default().verify(verifier()));
  let source = system.source().get_source_with(options);
  source.load_descriptor("app").await.unwrap();

  // Under the default (optional) policy the integrity check runs too and catches it.
  let options = BundleSourceOptions::default()
    .signature(BundleSourceSignatureOptions::default().verify(verifier()));
  let source = system.source().get_source_with(options);
  let err = source.load_descriptor("app").await.unwrap_err();
  assert!(matches!(err, wvb::Error::IntegrityVerifyFailed));
}

/// The updater and the source must agree: a bundle the updater accepted on download is the
/// bundle the source verifies on load, byte for byte.
#[tokio::test]
async fn a_downloaded_bundle_verifies_on_load() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0"))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(
      MockBundle::new("app", "2.0.0")
        .with_entry(INDEX, BundleEntry::new(b"<h1>v2</h1>", "text/html", None))
        .with_auto_integrity()
        .with_signed_integrity(sign),
    )
    .set_bundle_current_version("app", "2.0.0");

  let options = BundleSourceOptions::default()
    .integrity(BundleSourceIntegrityOptions::default().policy(IntegrityPolicy::Strict))
    .signature(BundleSourceSignatureOptions::default().verify(verifier()));
  let source = Arc::new(system.source().get_source_with(options));
  let remote = Arc::new(system.remote().get_remote());
  let updater = Updater::new(
    source.clone(),
    remote,
    Some(
      UpdaterConfig::default()
        .integrity_policy(IntegrityPolicy::Strict)
        .signature_verifier(verifier()),
    ),
  );

  updater.download("app", None).await.unwrap();
  updater.install("app", "2.0.0").await.unwrap();

  let protocol = BundleProtocol::new(source);
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>v2</h1>");
}

/// With the integrity check off, the signature is the only load-time check left. It runs on
/// the lazy read path (no bytes are read for it), so it must still fail closed — and the
/// once-per-version cache must not remember the failure as a success.
#[tokio::test]
async fn a_bad_signature_fails_the_load_even_with_integrity_off() {
  let bundle = app("1.0.0").with_auto_integrity().with_signature(sign(
    b"sha256:a-message-that-was-not-this-bundles-integrity",
  ));

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_remote_bundle(bundle)
    .set_remote_current_version("app", "1.0.0");

  let options = BundleSourceOptions::default()
    .integrity(BundleSourceIntegrityOptions::default().policy(IntegrityPolicy::Off))
    .signature(BundleSourceSignatureOptions::default().verify(verifier()));
  let source = system.source().get_source_with(options);

  let err = source.load_descriptor("app").await.unwrap_err();
  assert!(matches!(err, wvb::Error::SignatureVerifyFailed));
  // A retry re-runs the check rather than caching the error as a loaded descriptor.
  let err = source.load_descriptor("app").await.unwrap_err();
  assert!(matches!(err, wvb::Error::SignatureVerifyFailed));
}

/// `verify_mode` selects which bundle kinds have their signature checked, independently of
/// the integrity mode: `All` reaches builtin bundles too.
#[tokio::test]
async fn signature_verify_mode_all_reaches_builtin_bundles() {
  let bundle = app("1.0.0")
    .with_auto_integrity()
    .with_signature(sign(b"sha256:not-this-bundles-integrity"));

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(bundle)
    .set_builtin_current_version("app", "1.0.0");

  let options = BundleSourceOptions::default().signature(
    BundleSourceSignatureOptions::default()
      .verify(verifier())
      .verify_mode(BundleSourceSignatureVerifyMode::All),
  );
  let source = system.source().get_source_with(options);
  let err = source.load_descriptor("app").await.unwrap_err();
  assert!(matches!(err, wvb::Error::SignatureVerifyFailed));
}

/// ...whereas the default `OnlyRemote` leaves a builtin bundle's signature unchecked.
#[tokio::test]
async fn signature_verify_mode_only_remote_leaves_builtin_bundles_alone() {
  let bundle = app("1.0.0")
    .with_auto_integrity()
    .with_signature(sign(b"sha256:not-this-bundles-integrity"));

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(bundle)
    .set_builtin_current_version("app", "1.0.0");

  let options = BundleSourceOptions::default()
    .signature(BundleSourceSignatureOptions::default().verify(verifier()));
  let source = system.source().get_source_with(options);
  source.load_descriptor("app").await.unwrap();
}

/// The updater verifies the signature on download independently of the integrity policy, so
/// a bad signature is rejected even when the policy is off.
#[tokio::test]
async fn updater_rejects_a_bad_signature_on_download_with_integrity_off() {
  let mut system = MockSystem::new();
  system
    .remote_mut()
    .add_bundle(
      MockBundle::new("app", "2.0.0")
        .with_entry(INDEX, BundleEntry::new(b"<h1>v2</h1>", "text/html", None))
        .with_auto_integrity()
        .with_signature(sign(b"sha256:not-this-bundles-integrity")),
    )
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let updater = Updater::new(
    source,
    remote,
    Some(
      UpdaterConfig::default()
        .integrity_policy(IntegrityPolicy::Off)
        .signature_verifier(verifier()),
    ),
  );

  let err = updater.download("app", None).await.unwrap_err();
  assert!(matches!(err, wvb::Error::SignatureVerifyFailed));
}

/// With the integrity policy off, the updater installs a validly-signed bundle even when its
/// advertised integrity does not describe its bytes (the decoupling). The source's own
/// default-policy integrity check on load is what closes that gap.
#[tokio::test]
async fn updater_installs_a_signed_but_mismatched_bundle_only_for_the_load_to_reject_it() {
  let integrity_of_other_bytes = MockBundle::new("app", "2.0.0")
    .with_entry(INDEX, BundleEntry::new(b"other", "text/html", None))
    .with_auto_integrity();
  let advertised = integrity_of_other_bytes.integrity().unwrap().to_string();

  let mut system = MockSystem::new();
  system
    .remote_mut()
    .add_bundle(
      MockBundle::new("app", "2.0.0")
        .with_entry(INDEX, BundleEntry::new(b"<h1>v2</h1>", "text/html", None))
        .with_integrity(advertised.clone())
        .with_signature(sign(advertised.as_bytes())),
    )
    .set_bundle_current_version("app", "2.0.0");

  // The source verifies on load under its default (optional) policy.
  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let updater = Updater::new(
    source.clone(),
    remote,
    Some(
      UpdaterConfig::default()
        .integrity_policy(IntegrityPolicy::Off)
        .signature_verifier(verifier()),
    ),
  );

  // Integrity off + a valid signature over the advertised string: the updater accepts it.
  updater.download("app", None).await.unwrap();
  updater.install("app", "2.0.0").await.unwrap();

  // But loading it re-checks integrity, and the advertised hash does not match the bytes.
  let err = source.load_descriptor("app").await.unwrap_err();
  assert!(matches!(err, wvb::Error::IntegrityVerifyFailed));
}
