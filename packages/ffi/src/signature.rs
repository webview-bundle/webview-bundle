use std::sync::Arc;
use wvb::signature;

#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum SignatureAlgorithm {
  EcdsaSecp256r1,
  EcdsaSecp384r1,
  Ed25519,
  RsaPkcs1V15,
  RsaPss,
}

#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum VerifyingKeyFormat {
  SpkiDer,
  SpkiPem,
  Pkcs1Der,
  Pkcs1Pem,
  Sec1,
  Raw,
}

/// Key data for signature verification.
/// Use `pem` for PEM-encoded text keys, `der` for DER/raw binary keys.
#[derive(uniffi::Record, Clone, Debug)]
pub struct SignatureVerifyingKey {
  pub format: VerifyingKeyFormat,
  pub pem: Option<String>,
  pub der: Option<Vec<u8>>,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct SignatureVerifierOptions {
  pub algorithm: SignatureAlgorithm,
  pub key: SignatureVerifyingKey,
}

impl TryFrom<SignatureVerifierOptions> for signature::SignatureVerifier {
  type Error = crate::Error;

  fn try_from(opts: SignatureVerifierOptions) -> Result<Self, Self::Error> {
    let unsupported = crate::Error::Signature("unsupported key format for algorithm".to_string());

    fn require_pem(key: &SignatureVerifyingKey) -> Result<&str, crate::Error> {
      key
        .pem
        .as_deref()
        .ok_or_else(|| crate::Error::Signature("PEM key required".to_string()))
    }

    fn require_der(key: &SignatureVerifyingKey) -> Result<&[u8], crate::Error> {
      key
        .der
        .as_deref()
        .ok_or_else(|| crate::Error::Signature("DER key required".to_string()))
    }

    let verifier = match opts.algorithm {
      SignatureAlgorithm::EcdsaSecp256r1 => match opts.key.format {
        VerifyingKeyFormat::Sec1 => signature::SignatureVerifier::EcdsaSecp256r1(Arc::new(
          signature::EcdsaSecp256r1Verifier::from_sec1_bytes(require_der(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        VerifyingKeyFormat::SpkiDer => signature::SignatureVerifier::EcdsaSecp256r1(Arc::new(
          signature::EcdsaSecp256r1Verifier::from_public_key_der(require_der(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        VerifyingKeyFormat::SpkiPem => signature::SignatureVerifier::EcdsaSecp256r1(Arc::new(
          signature::EcdsaSecp256r1Verifier::from_public_key_pem(require_pem(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        _ => return Err(unsupported),
      },
      SignatureAlgorithm::EcdsaSecp384r1 => match opts.key.format {
        VerifyingKeyFormat::Sec1 => signature::SignatureVerifier::EcdsaSecp384r1(Arc::new(
          signature::EcdsaSecp384r1Verifier::from_sec1_bytes(require_der(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        VerifyingKeyFormat::SpkiDer => signature::SignatureVerifier::EcdsaSecp384r1(Arc::new(
          signature::EcdsaSecp384r1Verifier::from_public_key_der(require_der(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        VerifyingKeyFormat::SpkiPem => signature::SignatureVerifier::EcdsaSecp384r1(Arc::new(
          signature::EcdsaSecp384r1Verifier::from_public_key_pem(require_pem(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        _ => return Err(unsupported),
      },
      SignatureAlgorithm::Ed25519 => match opts.key.format {
        VerifyingKeyFormat::SpkiDer => signature::SignatureVerifier::Ed25519(Arc::new(
          signature::Ed25519Verifier::from_public_key_der(require_der(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        VerifyingKeyFormat::SpkiPem => signature::SignatureVerifier::Ed25519(Arc::new(
          signature::Ed25519Verifier::from_public_key_pem(require_pem(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        VerifyingKeyFormat::Raw => {
          let bytes = require_der(&opts.key)?;
          let arr: &[u8; 32] = bytes
            .try_into()
            .map_err(|_| crate::Error::Signature("Ed25519 raw key must be 32 bytes".to_string()))?;
          signature::SignatureVerifier::Ed25519(Arc::new(
            signature::Ed25519Verifier::from_public_key_bytes(arr).map_err(wvb::Error::from)?,
          ))
        }
        _ => return Err(unsupported),
      },
      SignatureAlgorithm::RsaPkcs1V15 => match opts.key.format {
        VerifyingKeyFormat::Pkcs1Der => signature::SignatureVerifier::RsaPkcs1V15(Arc::new(
          signature::RsaPkcs1V15Verifier::from_pkcs1_der(require_der(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        VerifyingKeyFormat::Pkcs1Pem => signature::SignatureVerifier::RsaPkcs1V15(Arc::new(
          signature::RsaPkcs1V15Verifier::from_pkcs1_pem(require_pem(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        VerifyingKeyFormat::SpkiDer => signature::SignatureVerifier::RsaPkcs1V15(Arc::new(
          signature::RsaPkcs1V15Verifier::from_public_key_der(require_der(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        VerifyingKeyFormat::SpkiPem => signature::SignatureVerifier::RsaPkcs1V15(Arc::new(
          signature::RsaPkcs1V15Verifier::from_public_key_pem(require_pem(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        _ => return Err(unsupported),
      },
      SignatureAlgorithm::RsaPss => match opts.key.format {
        VerifyingKeyFormat::Pkcs1Der => signature::SignatureVerifier::RsaPss(Arc::new(
          signature::RsaPssVerifier::from_pkcs1_der(require_der(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        VerifyingKeyFormat::Pkcs1Pem => signature::SignatureVerifier::RsaPss(Arc::new(
          signature::RsaPssVerifier::from_pkcs1_pem(require_pem(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        VerifyingKeyFormat::SpkiDer => signature::SignatureVerifier::RsaPss(Arc::new(
          signature::RsaPssVerifier::from_public_key_der(require_der(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        VerifyingKeyFormat::SpkiPem => signature::SignatureVerifier::RsaPss(Arc::new(
          signature::RsaPssVerifier::from_public_key_pem(require_pem(&opts.key)?)
            .map_err(wvb::Error::from)?,
        )),
        _ => return Err(unsupported),
      },
    };
    Ok(verifier)
  }
}
