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
use wvb::source::{BundleSourceOptions, VerifyOnLoad};
use wvb::testing::*;
use wvb::updater::{Updater, UpdaterConfig};
use wvb::{BundleBuilderOptions, BundleEntry, DataReadOptions};

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
    .with_options(BundleProtocolOptions::new().verify_data_checksum(false));
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), BODY);
}

/// A bundle packed with a non-zero seed must be served with the same seed.
#[tokio::test]
async fn protocol_honours_the_checksum_seed() {
  let mut options = BundleBuilderOptions::new();
  options.data_checksum_seed(42);

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0").with_builder_options(options))
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());

  let matching = BundleProtocol::new(source.clone())
    .with_options(BundleProtocolOptions::new().data_checksum_seed(42));
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

  let options = BundleSourceOptions::new().data(DataReadOptions::new().verify_checksum(true));
  let source = system.source().get_source_with(options);
  let descriptor = source.load_descriptor("app").await.unwrap();
  let err = descriptor.get_data(INDEX).await.unwrap_err();
  assert!(matches!(err, wvb::Error::ChecksumMismatch));
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

  let options = BundleSourceOptions::new()
    .verify_on_load(VerifyOnLoad::Remote)
    .integrity_policy(IntegrityPolicy::Strict);
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

  let options = BundleSourceOptions::new()
    .verify_on_load(VerifyOnLoad::Remote)
    .integrity_policy(IntegrityPolicy::Strict);
  let source = Arc::new(system.source().get_source_with(options));

  let protocol = BundleProtocol::new(source);
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), BODY);
}

/// `VerifyOnLoad::Remote` must not require integrity metadata on builtin bundles, which is
/// the whole reason it is not a plain on/off flag: builtin manifests carry no integrity
/// unless the app was packed with it.
#[tokio::test]
async fn verify_on_load_remote_leaves_builtin_bundles_alone() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0")) // no integrity metadata
    .set_builtin_current_version("app", "1.0.0");

  let options = BundleSourceOptions::new()
    .verify_on_load(VerifyOnLoad::Remote)
    .integrity_policy(IntegrityPolicy::Strict);
  let source = Arc::new(system.source().get_source_with(options));

  let protocol = BundleProtocol::new(source);
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), BODY);
}

/// ...whereas `All` does verify them, and so fails when the integrity is missing.
#[tokio::test]
async fn verify_on_load_all_requires_builtin_integrity() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(app("1.0.0"))
    .set_builtin_current_version("app", "1.0.0");

  let options = BundleSourceOptions::new()
    .verify_on_load(VerifyOnLoad::All)
    .integrity_policy(IntegrityPolicy::Strict);
  let source = system.source().get_source_with(options);

  let err = source.load_descriptor("app").await.unwrap_err();
  assert!(matches!(err, wvb::Error::IntegrityVerifyFailed));
}

#[tokio::test]
async fn verify_on_load_defaults_to_off() {
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

  // Loading succeeds: nothing hashes the file. The corruption is still caught on read, by
  // the entry checksum — the two layers are complementary.
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

  let options = BundleSourceOptions::new()
    .verify_on_load(VerifyOnLoad::Remote)
    .signature_verifier(verifier());
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

  let options = BundleSourceOptions::new()
    .verify_on_load(VerifyOnLoad::Remote)
    .signature_verifier(verifier());
  let source = system.source().get_source_with(options);

  let err = source.load_descriptor("app").await.unwrap_err();
  assert!(matches!(err, wvb::Error::SignatureVerifyFailed));
}

/// Configuring a verifier makes the integrity check mandatory even under
/// `IntegrityPolicy::None`: a signature over an unchecked hash proves nothing.
#[tokio::test]
async fn a_signature_verifier_forces_the_integrity_check() {
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

  let options = BundleSourceOptions::new()
    .verify_on_load(VerifyOnLoad::Remote)
    .integrity_policy(IntegrityPolicy::None)
    .signature_verifier(verifier());
  let source = system.source().get_source_with(options);

  // The signature is valid over the advertised integrity string — but that string does not
  // describe this bundle's bytes.
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

  let options = BundleSourceOptions::new()
    .verify_on_load(VerifyOnLoad::Remote)
    .integrity_policy(IntegrityPolicy::Strict)
    .signature_verifier(verifier());
  let source = Arc::new(system.source().get_source_with(options));
  let remote = Arc::new(system.remote().get_remote());
  let updater = Updater::new(
    source.clone(),
    remote,
    Some(
      UpdaterConfig::new()
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
