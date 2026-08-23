#![allow(dead_code)]

use base64ct::{Base64, Encoding};
use serde::{Deserialize, Serialize};
use specta::Type;
use wvb::signature::{
  self, EcdsaSecp256r1, EcdsaSecp384r1, Ed25519, RsaPkcs1V15Sha256, RsaPssSha256,
};

/// Digital signature algorithm for bundle verification. The wire strings match `@wvb/node`'s
/// `SignatureAlgorithm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum SignatureAlgorithm {
  #[serde(rename = "rsa-pkcs1-v1_5-sha256")]
  RsaPkcs1V1_5Sha256,
  #[serde(rename = "rsa-pss-sha256")]
  RsaPssSha256,
  #[serde(rename = "ecdsa-secp256r1")]
  EcdsaSecp256r1,
  #[serde(rename = "ecdsa-secp384r1")]
  EcdsaSecp384r1,
  #[serde(rename = "ed25519")]
  Ed25519,
}

/// Format of the public key used for signature verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum SignatureKeyFormat {
  #[serde(rename = "spki_der")]
  SpkiDer,
  #[serde(rename = "spki_pem")]
  SpkiPem,
  #[serde(rename = "pkcs1_der")]
  Pkcs1Der,
  #[serde(rename = "pkcs1_pem")]
  Pkcs1Pem,
  #[serde(rename = "sec1")]
  Sec1,
  #[serde(rename = "raw")]
  Raw,
}

impl SignatureKeyFormat {
  /// Whether the format carries raw bytes (base64 on the wire) rather than PEM text.
  fn is_binary(self) -> bool {
    !matches!(self, Self::SpkiPem | Self::Pkcs1Pem)
  }
}

/// The public key itself. `data` is the PEM text for the PEM formats, and standard base64 for the
/// binary ones (`spki_der`/`pkcs1_der`/`sec1`/`raw`) — `lib/signature.ts` encodes the `Uint8Array`
/// a caller passes.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignatureKeyData {
  pub format: SignatureKeyFormat,
  pub data: String,
}

impl SignatureKeyData {
  fn bytes(&self) -> Result<Vec<u8>, String> {
    Base64::decode_vec(&self.data).map_err(|_| "signature key is not valid base64".to_string())
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignatureKey {
  pub algorithm: SignatureAlgorithm,
  pub key: SignatureKeyData,
}

/// A verifying key paired with the id it is published under, so an update naming a `keyId` can be
/// matched to the key that verifies it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignatureVerifyKey {
  pub id: String,
  pub verify: SignatureKey,
}

fn unsupported(algorithm: SignatureAlgorithm, format: SignatureKeyFormat) -> String {
  format!("unsupported key format {format:?} for algorithm {algorithm:?}")
}

impl TryFrom<&SignatureKey> for signature::SignatureVerify {
  type Error = String;

  fn try_from(value: &SignatureKey) -> Result<Self, Self::Error> {
    let SignatureKey { algorithm, key } = value;
    let invalid = |e: wvb::Error| e.to_string();
    let verify = match (algorithm, key.format) {
      (SignatureAlgorithm::EcdsaSecp256r1, SignatureKeyFormat::Sec1) => {
        Self::EcdsaSecp256r1(EcdsaSecp256r1::from_sec1_bytes(&key.bytes()?).map_err(invalid)?)
      }
      (SignatureAlgorithm::EcdsaSecp256r1, SignatureKeyFormat::SpkiDer) => {
        Self::EcdsaSecp256r1(EcdsaSecp256r1::from_public_key_der(&key.bytes()?).map_err(invalid)?)
      }
      (SignatureAlgorithm::EcdsaSecp256r1, SignatureKeyFormat::SpkiPem) => {
        Self::EcdsaSecp256r1(EcdsaSecp256r1::from_public_key_pem(&key.data).map_err(invalid)?)
      }
      (SignatureAlgorithm::EcdsaSecp384r1, SignatureKeyFormat::Sec1) => {
        Self::EcdsaSecp384r1(EcdsaSecp384r1::from_sec1_bytes(&key.bytes()?).map_err(invalid)?)
      }
      (SignatureAlgorithm::EcdsaSecp384r1, SignatureKeyFormat::SpkiDer) => {
        Self::EcdsaSecp384r1(EcdsaSecp384r1::from_public_key_der(&key.bytes()?).map_err(invalid)?)
      }
      (SignatureAlgorithm::EcdsaSecp384r1, SignatureKeyFormat::SpkiPem) => {
        Self::EcdsaSecp384r1(EcdsaSecp384r1::from_public_key_pem(&key.data).map_err(invalid)?)
      }
      (SignatureAlgorithm::Ed25519, SignatureKeyFormat::SpkiDer) => {
        Self::Ed25519(Ed25519::from_public_key_der(&key.bytes()?).map_err(invalid)?)
      }
      (SignatureAlgorithm::Ed25519, SignatureKeyFormat::SpkiPem) => {
        Self::Ed25519(Ed25519::from_public_key_pem(&key.data).map_err(invalid)?)
      }
      (SignatureAlgorithm::Ed25519, SignatureKeyFormat::Raw) => {
        // Ed25519 raw keys must be exactly 32 bytes; reject anything else (fail closed).
        let bytes: [u8; 32] = key
          .bytes()?
          .as_slice()
          .try_into()
          .map_err(|_| "expect 32 bytes for an ed25519 raw key".to_string())?;
        Self::Ed25519(Ed25519::from_public_key_bytes(&bytes).map_err(invalid)?)
      }
      (SignatureAlgorithm::RsaPkcs1V1_5Sha256, SignatureKeyFormat::Pkcs1Der) => {
        Self::RsaPkcs1V1_5Sha256(RsaPkcs1V15Sha256::from_pkcs1_der(&key.bytes()?).map_err(invalid)?)
      }
      (SignatureAlgorithm::RsaPkcs1V1_5Sha256, SignatureKeyFormat::Pkcs1Pem) => {
        Self::RsaPkcs1V1_5Sha256(RsaPkcs1V15Sha256::from_pkcs1_pem(&key.data).map_err(invalid)?)
      }
      (SignatureAlgorithm::RsaPkcs1V1_5Sha256, SignatureKeyFormat::SpkiDer) => {
        Self::RsaPkcs1V1_5Sha256(
          RsaPkcs1V15Sha256::from_public_key_der(&key.bytes()?).map_err(invalid)?,
        )
      }
      (SignatureAlgorithm::RsaPkcs1V1_5Sha256, SignatureKeyFormat::SpkiPem) => {
        Self::RsaPkcs1V1_5Sha256(
          RsaPkcs1V15Sha256::from_public_key_pem(&key.data).map_err(invalid)?,
        )
      }
      (SignatureAlgorithm::RsaPssSha256, SignatureKeyFormat::Pkcs1Der) => {
        Self::RsaPssSha256(RsaPssSha256::from_pkcs1_der(&key.bytes()?).map_err(invalid)?)
      }
      (SignatureAlgorithm::RsaPssSha256, SignatureKeyFormat::Pkcs1Pem) => {
        Self::RsaPssSha256(RsaPssSha256::from_pkcs1_pem(&key.data).map_err(invalid)?)
      }
      (SignatureAlgorithm::RsaPssSha256, SignatureKeyFormat::SpkiDer) => {
        Self::RsaPssSha256(RsaPssSha256::from_public_key_der(&key.bytes()?).map_err(invalid)?)
      }
      (SignatureAlgorithm::RsaPssSha256, SignatureKeyFormat::SpkiPem) => {
        Self::RsaPssSha256(RsaPssSha256::from_public_key_pem(&key.data).map_err(invalid)?)
      }
      (algorithm, format) => return Err(unsupported(*algorithm, format)),
    };
    Ok(verify)
  }
}

impl TryFrom<&SignatureVerifyKey> for signature::SignatureVerifyKey {
  type Error = String;

  fn try_from(value: &SignatureVerifyKey) -> Result<Self, Self::Error> {
    Ok(Self {
      id: value.id.clone(),
      verify: (&value.verify).try_into()?,
    })
  }
}

/// Whether `format` expects `data` to be base64-encoded bytes. Kept next to the wire type so
/// `lib/signature.ts`'s matching check has one place to follow.
pub(crate) fn format_is_binary(format: SignatureKeyFormat) -> bool {
  format.is_binary()
}

#[cfg(test)]
mod tests {
  use super::*;

  const ED25519_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAzUROGx/OqiO9ZwxWsaG3ChmBqEGpXKTC9DmAVx86J5E=\n-----END PUBLIC KEY-----";

  fn key(algorithm: SignatureAlgorithm, format: SignatureKeyFormat, data: &str) -> SignatureKey {
    SignatureKey {
      algorithm,
      key: SignatureKeyData {
        format,
        data: data.to_string(),
      },
    }
  }

  #[test]
  fn builds_an_ed25519_key_from_pem() {
    let key = key(
      SignatureAlgorithm::Ed25519,
      SignatureKeyFormat::SpkiPem,
      ED25519_PEM,
    );
    assert!(signature::SignatureVerify::try_from(&key).is_ok());
  }

  #[test]
  fn fails_closed_on_a_bad_key() {
    // Too short to be an ed25519 public key: must not fall back to unverified.
    let short = key(SignatureAlgorithm::Ed25519, SignatureKeyFormat::Raw, "AAAA");
    assert!(signature::SignatureVerify::try_from(&short).is_err());

    let not_pem = key(
      SignatureAlgorithm::Ed25519,
      SignatureKeyFormat::SpkiPem,
      "not a valid key",
    );
    assert!(signature::SignatureVerify::try_from(&not_pem).is_err());
  }

  #[test]
  fn rejects_an_unsupported_algorithm_format_pair() {
    let sec1 = key(
      SignatureAlgorithm::Ed25519,
      SignatureKeyFormat::Sec1,
      "AAAA",
    );
    assert!(signature::SignatureVerify::try_from(&sec1).is_err());
  }
}
