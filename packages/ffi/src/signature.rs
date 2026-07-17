use std::sync::Arc;
use wvb::signature;

/// A custom function that verifies a bundle's signature.
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait SignatureVerify: Send + Sync {
  async fn verify(&self, message: Vec<u8>, signature: String) -> bool;
}

pub(crate) fn into_verifier(verify: Arc<dyn SignatureVerify>) -> signature::SignatureVerify {
  signature::SignatureVerify::Custom(Arc::new(move |message: &[u8], signature: &str| {
    let verify = verify.clone();
    let message = message.to_vec();
    let signature = signature.to_string();
    Box::pin(async move {
      Ok::<bool, Box<dyn std::error::Error + Send + Sync + 'static>>(
        verify.verify(message, signature).await,
      )
    })
  }))
}

/// How a bundle's signature is verified: with a declarative public key, or a custom function.
#[derive(uniffi::Enum, Clone)]
pub enum SignatureVerification {
  /// Verify with a public key of a known algorithm.
  Key { options: SignatureVerifierOptions },
  /// Verify with a custom function over the integrity string.
  Custom { verify: Arc<dyn SignatureVerify> },
}

impl TryFrom<SignatureVerification> for signature::SignatureVerify {
  type Error = crate::Error;

  fn try_from(value: SignatureVerification) -> Result<Self, Self::Error> {
    match value {
      SignatureVerification::Key { options } => Self::try_from(options),
      SignatureVerification::Custom { verify } => Ok(into_verifier(verify)),
    }
  }
}

/// Digital signature algorithm used to verify bundle authenticity.
#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum SignatureAlgorithm {
  EcdsaSecp256r1,
  EcdsaSecp384r1,
  Ed25519,
  RsaPkcs1V15,
  RsaPss,
}

/// Encoding format of the public key provided in [`SignatureVerifyingKey`].
///
/// Not all combinations of algorithm + format are valid; unsupported pairs
/// return [`Error::BindingInvalidSignatureOptions`] at construction time.
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

/// Configuration passed to the updater to enable signature verification.
#[derive(uniffi::Record, Clone, Debug)]
pub struct SignatureVerifierOptions {
  pub algorithm: SignatureAlgorithm,
  pub key: SignatureVerifyingKey,
}

impl TryFrom<SignatureVerifierOptions> for signature::SignatureVerify {
  type Error = crate::Error;

  fn try_from(opts: SignatureVerifierOptions) -> Result<Self, Self::Error> {
    let unsupported =
      crate::Error::invalid_signature_options("unsupported key format for algorithm");

    fn require_pem(key: &SignatureVerifyingKey) -> Result<&str, crate::Error> {
      key
        .pem
        .as_deref()
        .ok_or_else(|| crate::Error::invalid_signature_options("PEM key required"))
    }

    fn require_der(key: &SignatureVerifyingKey) -> Result<&[u8], crate::Error> {
      key
        .der
        .as_deref()
        .ok_or_else(|| crate::Error::invalid_signature_options("DER key required"))
    }

    let verifier = match opts.algorithm {
      SignatureAlgorithm::EcdsaSecp256r1 => match opts.key.format {
        VerifyingKeyFormat::Sec1 => signature::SignatureVerify::EcdsaSecp256r1(Arc::new(
          signature::EcdsaSecp256r1Verifier::from_sec1_bytes(require_der(&opts.key)?)?,
        )),
        VerifyingKeyFormat::SpkiDer => signature::SignatureVerify::EcdsaSecp256r1(Arc::new(
          signature::EcdsaSecp256r1Verifier::from_public_key_der(require_der(&opts.key)?)?,
        )),
        VerifyingKeyFormat::SpkiPem => signature::SignatureVerify::EcdsaSecp256r1(Arc::new(
          signature::EcdsaSecp256r1Verifier::from_public_key_pem(require_pem(&opts.key)?)?,
        )),
        _ => return Err(unsupported),
      },
      SignatureAlgorithm::EcdsaSecp384r1 => match opts.key.format {
        VerifyingKeyFormat::Sec1 => signature::SignatureVerify::EcdsaSecp384r1(Arc::new(
          signature::EcdsaSecp384r1Verifier::from_sec1_bytes(require_der(&opts.key)?)?,
        )),
        VerifyingKeyFormat::SpkiDer => signature::SignatureVerify::EcdsaSecp384r1(Arc::new(
          signature::EcdsaSecp384r1Verifier::from_public_key_der(require_der(&opts.key)?)?,
        )),
        VerifyingKeyFormat::SpkiPem => signature::SignatureVerify::EcdsaSecp384r1(Arc::new(
          signature::EcdsaSecp384r1Verifier::from_public_key_pem(require_pem(&opts.key)?)?,
        )),
        _ => return Err(unsupported),
      },
      SignatureAlgorithm::Ed25519 => match opts.key.format {
        VerifyingKeyFormat::SpkiDer => signature::SignatureVerify::Ed25519(Arc::new(
          signature::Ed25519Verifier::from_public_key_der(require_der(&opts.key)?)?,
        )),
        VerifyingKeyFormat::SpkiPem => signature::SignatureVerify::Ed25519(Arc::new(
          signature::Ed25519Verifier::from_public_key_pem(require_pem(&opts.key)?)?,
        )),
        VerifyingKeyFormat::Raw => {
          let bytes = require_der(&opts.key)?;
          let arr: &[u8; 32] = bytes.try_into().map_err(|_| {
            crate::Error::invalid_signature_options("Ed25519 raw key must be 32 bytes")
          })?;
          signature::SignatureVerify::Ed25519(Arc::new(
            signature::Ed25519Verifier::from_public_key_bytes(arr)?,
          ))
        }
        _ => return Err(unsupported),
      },
      SignatureAlgorithm::RsaPkcs1V15 => match opts.key.format {
        VerifyingKeyFormat::Pkcs1Der => signature::SignatureVerify::RsaPkcs1V15(Arc::new(
          signature::RsaPkcs1V15Verifier::from_pkcs1_der(require_der(&opts.key)?)?,
        )),
        VerifyingKeyFormat::Pkcs1Pem => signature::SignatureVerify::RsaPkcs1V15(Arc::new(
          signature::RsaPkcs1V15Verifier::from_pkcs1_pem(require_pem(&opts.key)?)?,
        )),
        VerifyingKeyFormat::SpkiDer => signature::SignatureVerify::RsaPkcs1V15(Arc::new(
          signature::RsaPkcs1V15Verifier::from_public_key_der(require_der(&opts.key)?)?,
        )),
        VerifyingKeyFormat::SpkiPem => signature::SignatureVerify::RsaPkcs1V15(Arc::new(
          signature::RsaPkcs1V15Verifier::from_public_key_pem(require_pem(&opts.key)?)?,
        )),
        _ => return Err(unsupported),
      },
      SignatureAlgorithm::RsaPss => match opts.key.format {
        VerifyingKeyFormat::Pkcs1Der => signature::SignatureVerify::RsaPss(Arc::new(
          signature::RsaPssVerifier::from_pkcs1_der(require_der(&opts.key)?)?,
        )),
        VerifyingKeyFormat::Pkcs1Pem => signature::SignatureVerify::RsaPss(Arc::new(
          signature::RsaPssVerifier::from_pkcs1_pem(require_pem(&opts.key)?)?,
        )),
        VerifyingKeyFormat::SpkiDer => signature::SignatureVerify::RsaPss(Arc::new(
          signature::RsaPssVerifier::from_public_key_der(require_der(&opts.key)?)?,
        )),
        VerifyingKeyFormat::SpkiPem => signature::SignatureVerify::RsaPss(Arc::new(
          signature::RsaPssVerifier::from_public_key_pem(require_pem(&opts.key)?)?,
        )),
        _ => return Err(unsupported),
      },
    };
    Ok(verifier)
  }
}
