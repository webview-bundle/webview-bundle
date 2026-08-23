use crate::js::{JsCallback, JsCallbackExt};
use napi::bindgen_prelude::{Buffer, FnArgs, FromNapiValue, Promise, TypeName, ValidateNapiValue};
use napi::{Either, ValueType, sys};
use napi_derive::napi;
use std::sync::Arc;
use wvb::signature;

/// Digital signature algorithm for bundle verification.
///
/// Supports multiple signature schemes for cryptographic verification of bundle authenticity.
#[napi(string_enum)]
#[derive(PartialEq, Eq)]
pub enum SignatureAlgorithm {
  #[napi(value = "rsa-pkcs1-v1_5-sha256")]
  /// RSA PKCS#1 v1.5 signature scheme
  RsaPkcs1V1_5Sha256,
  #[napi(value = "rsa-pss-sha256")]
  /// RSA-PSS (Probabilistic Signature Scheme)
  RsaPssSha256,
  #[napi(value = "ecdsa-secp256r1")]
  /// ECDSA with P-256 curve (secp256r1)
  EcdsaSecp256r1,
  #[napi(value = "ecdsa-secp384r1")]
  /// ECDSA with P-384 curve (secp384r1)
  EcdsaSecp384r1,
  #[napi(value = "ed25519")]
  /// Ed25519
  Ed25519,
}

/// Format of the public key used for signature verification.
///
/// Different algorithms support different key formats.
///
/// @example
/// ```typescript
/// import fs from 'fs';
///
/// // PEM format (text)
/// const pemKey = fs.readFileSync('./public-key.pem', 'utf8');
/// const config1 = {
///   format: 'spki_pem',
///   data: pemKey,
/// };
///
/// // DER format (binary)
/// const derKey = fs.readFileSync('./public-key.der');
/// const config2 = {
///   format: 'spki_der',
///   data: derKey,
/// };
///
/// // Raw bytes (Ed25519 only)
/// const rawKey = new Uint8Array(32);
/// const config3 = {
///   format: 'raw',
///   data: rawKey,
/// };
/// ```
#[napi(string_enum)]
#[derive(PartialEq, Eq)]
pub enum SignatureKeyFormat {
  #[napi(value = "spki_der")]
  /// SubjectPublicKeyInfo DER format (binary)
  SpkiDer,
  #[napi(value = "spki_pem")]
  /// SubjectPublicKeyInfo PEM format (text)
  SpkiPem,
  #[napi(value = "pkcs1_der")]
  /// PKCS#1 DER format (RSA only, binary)
  Pkcs1Der,
  #[napi(value = "pkcs1_pem")]
  /// PKCS#1 PEM format (RSA only, text)
  Pkcs1Pem,
  #[napi(value = "sec1")]
  /// SEC1 format (ECDSA only, binary)
  Sec1,
  #[napi(value = "raw")]
  /// Raw key bytes (Ed25519 only, 32 bytes)
  Raw,
}

#[napi(object, object_to_js = false)]
pub struct SignatureVerifyKey {
  pub id: String,
  #[napi(
    ts_type = "SignatureKey | ((message: Uint8Array, signature: string) => Promise<boolean>)"
  )]
  pub verify: SignatureVerify,
}

impl From<SignatureVerifyKey> for signature::SignatureVerifyKey {
  fn from(value: SignatureVerifyKey) -> Self {
    Self {
      id: value.id,
      verify: value.verify.inner,
    }
  }
}

/// Signature key
pub struct SignatureVerify {
  pub(crate) inner: signature::SignatureVerify,
}

/// Configuration for signature verification.
///
/// @property {SignatureAlgorithm} algorithm - The signature algorithm to use
/// @property {SignatureKeyData} key - The public key configuration
#[napi(object, object_to_js = false)]
pub struct SignatureKey {
  pub algorithm: SignatureAlgorithm,
  pub key: SignatureKeyData,
}

/// Public key configuration for signature verification.
///
/// @property {VerifyingKeyFormat} format - The format of the public key
/// @property {string | Uint8Array} data - The key data (string for PEM, Uint8Array for DER/Raw)
#[napi(object, object_to_js = false)]
pub struct SignatureKeyData {
  pub format: SignatureKeyFormat,
  #[napi(ts_type = "string | Uint8Array")]
  pub data: Either<String, Buffer>,
}

type NapiSignatureKey = Either<SignatureKey, JsCallback<FnArgs<(Buffer, String)>, Promise<bool>>>;

impl TypeName for SignatureVerify {
  fn type_name() -> &'static str {
    NapiSignatureKey::type_name()
  }

  fn value_type() -> ValueType {
    NapiSignatureKey::value_type()
  }
}

impl ValidateNapiValue for SignatureVerify {
  unsafe fn validate(
    env: sys::napi_env,
    napi_val: sys::napi_value,
  ) -> napi::Result<sys::napi_value> {
    unsafe { NapiSignatureKey::validate(env, napi_val) }
  }
}

impl FromNapiValue for SignatureVerify {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    unsafe {
      let value = NapiSignatureKey::from_napi_value(env, napi_val)?;
      let unsupported_key_format = napi::Error::from(crate::Error::invalid_signature_key(
        "unsupported key format",
      ));
      let value = match value {
        Either::A(inner) => match &inner.algorithm {
          SignatureAlgorithm::EcdsaSecp256r1 => {
            let key = match &inner.key.format {
              SignatureKeyFormat::Sec1 => Ok(
                signature::EcdsaSecp256r1::from_sec1_bytes(&into_buffer_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              SignatureKeyFormat::SpkiDer => Ok(
                signature::EcdsaSecp256r1::from_public_key_der(&into_buffer_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              SignatureKeyFormat::SpkiPem => Ok(
                signature::EcdsaSecp256r1::from_public_key_pem(&into_string_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              _ => Err(unsupported_key_format),
            }?;
            signature::SignatureVerify::EcdsaSecp256r1(key)
          }
          SignatureAlgorithm::EcdsaSecp384r1 => {
            let key = match &inner.key.format {
              SignatureKeyFormat::Sec1 => Ok(
                signature::EcdsaSecp384r1::from_sec1_bytes(&into_buffer_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              SignatureKeyFormat::SpkiDer => Ok(
                signature::EcdsaSecp384r1::from_public_key_der(&into_buffer_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              SignatureKeyFormat::SpkiPem => Ok(
                signature::EcdsaSecp384r1::from_public_key_pem(&into_string_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              _ => Err(unsupported_key_format),
            }?;
            signature::SignatureVerify::EcdsaSecp384r1(key)
          }
          SignatureAlgorithm::Ed25519 => {
            let key = match &inner.key.format {
              SignatureKeyFormat::SpkiDer => Ok(
                signature::Ed25519::from_public_key_der(&into_buffer_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              SignatureKeyFormat::SpkiPem => Ok(
                signature::Ed25519::from_public_key_pem(&into_string_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              SignatureKeyFormat::Raw => {
                let data = into_buffer_data(inner.key.data)?;
                let bytes = data
                  .get(..32)
                  .and_then(|s| s.try_into().ok())
                  .ok_or_else(|| {
                    napi::Error::from(crate::Error::invalid_signature_key(
                      "Expect 32 bytes for key pair",
                    ))
                  })?;
                Ok(
                  signature::Ed25519::from_public_key_bytes(bytes)
                    .map_err(crate::Error::from)
                    .map_err(napi::Error::from)?,
                )
              }
              _ => Err(unsupported_key_format),
            }?;
            signature::SignatureVerify::Ed25519(key)
          }
          SignatureAlgorithm::RsaPkcs1V1_5Sha256 => {
            let key = match &inner.key.format {
              SignatureKeyFormat::Pkcs1Der => Ok(
                signature::RsaPkcs1V15Sha256::from_pkcs1_der(&into_buffer_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              SignatureKeyFormat::Pkcs1Pem => Ok(
                signature::RsaPkcs1V15Sha256::from_pkcs1_pem(&into_string_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              SignatureKeyFormat::SpkiDer => Ok(
                signature::RsaPkcs1V15Sha256::from_public_key_der(&into_buffer_data(
                  inner.key.data,
                )?)
                .map_err(crate::Error::from)
                .map_err(napi::Error::from)?,
              ),
              SignatureKeyFormat::SpkiPem => Ok(
                signature::RsaPkcs1V15Sha256::from_public_key_pem(&into_string_data(
                  inner.key.data,
                )?)
                .map_err(crate::Error::from)
                .map_err(napi::Error::from)?,
              ),
              _ => Err(unsupported_key_format),
            }?;
            signature::SignatureVerify::RsaPkcs1V1_5Sha256(key)
          }
          SignatureAlgorithm::RsaPssSha256 => {
            let key = match &inner.key.format {
              SignatureKeyFormat::Pkcs1Der => Ok(
                signature::RsaPssSha256::from_pkcs1_der(&into_buffer_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              SignatureKeyFormat::Pkcs1Pem => Ok(
                signature::RsaPssSha256::from_pkcs1_pem(&into_string_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              SignatureKeyFormat::SpkiDer => Ok(
                signature::RsaPssSha256::from_public_key_der(&into_buffer_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              SignatureKeyFormat::SpkiPem => Ok(
                signature::RsaPssSha256::from_public_key_pem(&into_string_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              _ => Err(unsupported_key_format),
            }?;
            signature::SignatureVerify::RsaPssSha256(key)
          }
        },
        Either::B(inner) => {
          signature::SignatureVerify::Custom(Arc::new(move |message, signature| {
            let message_buf = Buffer::from(message);
            let signature = signature.to_string();
            let callback = Arc::clone(&inner);
            Box::pin(async move {
              let ret = callback
                .invoke_async((message_buf, signature).into())
                .await?
                .await?;
              Ok(ret)
            })
          }))
        }
      };
      Ok(Self { inner: value })
    }
  }
}

// A key whose data type doesn't match its declared format is an invalid verifier option, so it
// surfaces the same `invalid_signature_options` code as every other verifier-construction failure
// (rather than the generic `napi` code an untagged `napi::Error` would yield).
fn into_string_data(d: Either<String, Buffer>) -> napi::Result<String> {
  match d {
    Either::A(s) => Ok(s),
    Either::B(_) => Err(
      crate::Error::invalid_signature_key("signature key must be a string for this format").into(),
    ),
  }
}

fn into_buffer_data(d: Either<String, Buffer>) -> napi::Result<Buffer> {
  match d {
    Either::A(_) => Err(
      crate::Error::invalid_signature_key("signature key must be a Buffer for this format").into(),
    ),
    Either::B(b) => Ok(b),
  }
}
