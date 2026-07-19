#![allow(dead_code)]

use base64ct::{Base64, Encoding};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use wvb::signature::{
  EcdsaSecp256r1Verifier, EcdsaSecp384r1Verifier, Ed25519Verifier, RsaPkcs1V15Verifier,
  RsaPssVerifier, SignatureVerify,
};

/// Digital signature algorithm for bundle verification. The wire strings match `@wvb/node`'s
/// napi-generated `SignatureAlgorithm` (note the capital `R` in `ecdsaSecp256R1`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
pub enum SignatureAlgorithm {
  #[serde(rename = "ecdsaSecp256R1")]
  EcdsaSecp256r1,
  #[serde(rename = "ecdsaSecp384R1")]
  EcdsaSecp384r1,
  #[serde(rename = "ed25519")]
  Ed25519,
  #[serde(rename = "rsaPkcs1V15")]
  RsaPkcs1V15,
  #[serde(rename = "rsaPss")]
  RsaPss,
}

/// Format of the public key used for signature verification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum VerifyingKeyFormat {
  SpkiDer,
  SpkiPem,
  Pkcs1Der,
  Pkcs1Pem,
  Sec1,
  Raw,
}

/// Build a core `SignatureVerifier` from a verifier JSON object (the source's `signature.verify`
/// or the updater's `signatureVerifier`): `{ algorithm, key: { format, data } }`.
///
/// For the PEM key formats `data` is the PEM text; for the binary formats (`spkiDer`/`pkcs1Der`
/// /`sec1`/`raw`) it is standard base64. Returns `None` on any parse, base64-decode, unsupported
/// algorithm/format combination, or key-construction failure, so the caller can fail closed.
pub(crate) fn build_signature_verifier(sv: &serde_json::Value) -> Option<SignatureVerify> {
  let algorithm = sv.get("algorithm")?.as_str()?;
  let key = sv.get("key")?;
  let format = key.get("format")?.as_str()?;
  let data = key.get("data")?.as_str()?;
  let bytes = || Base64::decode_vec(data).ok();
  let verifier = match (algorithm, format) {
    ("ecdsaSecp256R1", "sec1") => SignatureVerify::EcdsaSecp256r1(Arc::new(
      EcdsaSecp256r1Verifier::from_sec1_bytes(&bytes()?).ok()?,
    )),
    ("ecdsaSecp256R1", "spkiDer") => SignatureVerify::EcdsaSecp256r1(Arc::new(
      EcdsaSecp256r1Verifier::from_public_key_der(&bytes()?).ok()?,
    )),
    ("ecdsaSecp256R1", "spkiPem") => SignatureVerify::EcdsaSecp256r1(Arc::new(
      EcdsaSecp256r1Verifier::from_public_key_pem(data).ok()?,
    )),
    ("ecdsaSecp384R1", "sec1") => SignatureVerify::EcdsaSecp384r1(Arc::new(
      EcdsaSecp384r1Verifier::from_sec1_bytes(&bytes()?).ok()?,
    )),
    ("ecdsaSecp384R1", "spkiDer") => SignatureVerify::EcdsaSecp384r1(Arc::new(
      EcdsaSecp384r1Verifier::from_public_key_der(&bytes()?).ok()?,
    )),
    ("ecdsaSecp384R1", "spkiPem") => SignatureVerify::EcdsaSecp384r1(Arc::new(
      EcdsaSecp384r1Verifier::from_public_key_pem(data).ok()?,
    )),
    ("ed25519", "spkiDer") => SignatureVerify::Ed25519(Arc::new(
      Ed25519Verifier::from_public_key_der(&bytes()?).ok()?,
    )),
    ("ed25519", "spkiPem") => {
      SignatureVerify::Ed25519(Arc::new(Ed25519Verifier::from_public_key_pem(data).ok()?))
    }
    ("ed25519", "raw") => {
      let raw = bytes()?;
      // Ed25519 raw keys must be exactly 32 bytes; reject anything else (fail closed).
      let arr: [u8; 32] = raw.as_slice().try_into().ok()?;
      SignatureVerify::Ed25519(Arc::new(Ed25519Verifier::from_public_key_bytes(&arr).ok()?))
    }
    ("rsaPkcs1V15", "pkcs1Der") => SignatureVerify::RsaPkcs1V15(Arc::new(
      RsaPkcs1V15Verifier::from_pkcs1_der(&bytes()?).ok()?,
    )),
    ("rsaPkcs1V15", "pkcs1Pem") => {
      SignatureVerify::RsaPkcs1V15(Arc::new(RsaPkcs1V15Verifier::from_pkcs1_pem(data).ok()?))
    }
    ("rsaPkcs1V15", "spkiDer") => SignatureVerify::RsaPkcs1V15(Arc::new(
      RsaPkcs1V15Verifier::from_public_key_der(&bytes()?).ok()?,
    )),
    ("rsaPkcs1V15", "spkiPem") => SignatureVerify::RsaPkcs1V15(Arc::new(
      RsaPkcs1V15Verifier::from_public_key_pem(data).ok()?,
    )),
    ("rsaPss", "pkcs1Der") => {
      SignatureVerify::RsaPss(Arc::new(RsaPssVerifier::from_pkcs1_der(&bytes()?).ok()?))
    }
    ("rsaPss", "pkcs1Pem") => {
      SignatureVerify::RsaPss(Arc::new(RsaPssVerifier::from_pkcs1_pem(data).ok()?))
    }
    ("rsaPss", "spkiDer") => SignatureVerify::RsaPss(Arc::new(
      RsaPssVerifier::from_public_key_der(&bytes()?).ok()?,
    )),
    ("rsaPss", "spkiPem") => {
      SignatureVerify::RsaPss(Arc::new(RsaPssVerifier::from_public_key_pem(data).ok()?))
    }
    _ => return None,
  };
  Some(verifier)
}
