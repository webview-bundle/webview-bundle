use crate::js::{JsCallback, JsCallbackExt};
use napi::bindgen_prelude::{Buffer, FnArgs, FromNapiValue, Promise, TypeName, ValidateNapiValue};
use napi::{Either, ValueType, sys};
use napi_derive::napi;
use std::sync::Arc;
use wvb::signature;

/// Digital signature algorithm for bundle verification.
///
/// Supports multiple signature schemes for cryptographic verification of bundle authenticity.
///
/// @example
/// ```typescript
/// import { Updater, SignatureAlgorithm, VerifyingKeyFormat } from "@wvb/node";
///
/// const updater = new Updater(source, remote, {
///   signatureVerifier: {
///     algorithm: SignatureAlgorithm.Ed25519,
///     key: {
///       format: VerifyingKeyFormat.SpkiPem,
///       data: "-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----"
///     }
///   }
/// });
/// ```
#[napi(string_enum = "camelCase")]
#[derive(PartialEq, Eq)]
pub enum SignatureAlgorithm {
  /// ECDSA with P-256 curve (secp256r1)
  EcdsaSecp256r1,
  /// ECDSA with P-384 curve (secp384r1)
  EcdsaSecp384r1,
  /// Ed25519 (EdDSA, recommended for modern applications)
  Ed25519,
  /// RSA PKCS#1 v1.5 signature scheme
  RsaPkcs1V1_5,
  /// RSA-PSS (Probabilistic Signature Scheme)
  RsaPss,
}

/// Format of the public key used for signature verification.
///
/// Different algorithms support different key formats.
///
/// @example
/// ```typescript
/// import { VerifyingKeyFormat } from "@wvb/node";
/// import fs from "fs";
///
/// // PEM format (text)
/// const pemKey = fs.readFileSync("./public-key.pem", "utf8");
/// const config1 = {
///   format: VerifyingKeyFormat.SpkiPem,
///   data: pemKey
/// };
///
/// // DER format (binary)
/// const derKey = fs.readFileSync("./public-key.der");
/// const config2 = {
///   format: VerifyingKeyFormat.SpkiDer,
///   data: derKey
/// };
///
/// // Raw bytes (Ed25519 only)
/// const rawKey = new Uint8Array(32);
/// const config3 = {
///   format: VerifyingKeyFormat.Raw,
///   data: rawKey
/// };
/// ```
#[napi(string_enum = "camelCase")]
#[derive(PartialEq, Eq)]
pub enum VerifyingKeyFormat {
  /// SubjectPublicKeyInfo DER format (binary)
  SpkiDer,
  /// SubjectPublicKeyInfo PEM format (text)
  SpkiPem,
  /// PKCS#1 DER format (RSA only, binary)
  Pkcs1Der,
  /// PKCS#1 PEM format (RSA only, text)
  Pkcs1Pem,
  /// SEC1 format (ECDSA only, binary)
  Sec1,
  /// Raw key bytes (Ed25519 only, 32 bytes)
  Raw,
}

/// Signature verifier for bundle authenticity verification.
///
/// This type is used internally and can be created from either a configuration object
/// or a custom verification function.
pub struct SignatureVerifier {
  pub(crate) inner: signature::SignatureVerifier,
}

/// Configuration for signature verification.
///
/// @property {SignatureAlgorithm} algorithm - The signature algorithm to use
/// @property {SignatureVerifyingKeyOptions} key - The public key configuration
///
/// @example
/// ```typescript
/// const verifierOptions = {
///   algorithm: SignatureAlgorithm.Ed25519,
///   key: {
///     format: VerifyingKeyFormat.SpkiPem,
///     data: "-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----"
///   }
/// };
/// ```
#[napi(object, object_to_js = false)]
pub struct SignatureVerifierOptions {
  pub algorithm: SignatureAlgorithm,
  pub key: SignatureVerifyingKeyOptions,
}

/// Public key configuration for signature verification.
///
/// @property {VerifyingKeyFormat} format - The format of the public key
/// @property {string | Uint8Array} data - The key data (string for PEM, Uint8Array for DER/Raw)
///
/// @example
/// ```typescript
/// // PEM format (string)
/// const pemKey = {
///   format: VerifyingKeyFormat.SpkiPem,
///   data: "-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----"
/// };
///
/// // DER format (binary)
/// const derKey = {
///   format: VerifyingKeyFormat.SpkiDer,
///   data: new Uint8Array([...])
/// };
/// ```
#[napi(object, object_to_js = false)]
pub struct SignatureVerifyingKeyOptions {
  pub format: VerifyingKeyFormat,
  #[napi(ts_type = "string | Uint8Array")]
  pub data: Either<String, Buffer>,
}

type NapiVerifier =
  Either<SignatureVerifierOptions, JsCallback<FnArgs<(Buffer, String)>, Promise<bool>>>;

impl TypeName for SignatureVerifier {
  fn type_name() -> &'static str {
    NapiVerifier::type_name()
  }

  fn value_type() -> ValueType {
    NapiVerifier::value_type()
  }
}

impl ValidateNapiValue for SignatureVerifier {
  unsafe fn validate(
    env: sys::napi_env,
    napi_val: sys::napi_value,
  ) -> napi::Result<sys::napi_value> {
    unsafe { NapiVerifier::validate(env, napi_val) }
  }
}

impl FromNapiValue for SignatureVerifier {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    unsafe {
      let value = NapiVerifier::from_napi_value(env, napi_val)?;
      let unsupported_key_format =
        napi::Error::new(napi::Status::InvalidArg, "unsupported key format");
      let value = match value {
        Either::A(inner) => match &inner.algorithm {
          SignatureAlgorithm::EcdsaSecp256r1 => {
            let verifier = match &inner.key.format {
              VerifyingKeyFormat::Sec1 => Ok(
                signature::EcdsaSecp256r1Verifier::from_sec1_bytes(&into_buffer_data(
                  inner.key.data,
                )?)
                .map_err(crate::Error::from)
                .map_err(napi::Error::from)?,
              ),
              VerifyingKeyFormat::SpkiDer => Ok(
                signature::EcdsaSecp256r1Verifier::from_public_key_der(&into_buffer_data(
                  inner.key.data,
                )?)
                .map_err(crate::Error::from)
                .map_err(napi::Error::from)?,
              ),
              VerifyingKeyFormat::SpkiPem => Ok(
                signature::EcdsaSecp256r1Verifier::from_public_key_pem(&into_string_data(
                  inner.key.data,
                )?)
                .map_err(crate::Error::from)
                .map_err(napi::Error::from)?,
              ),
              _ => Err(unsupported_key_format),
            }?;
            signature::SignatureVerifier::EcdsaSecp256r1(Arc::new(verifier))
          }
          SignatureAlgorithm::EcdsaSecp384r1 => {
            let verifier = match &inner.key.format {
              VerifyingKeyFormat::Sec1 => Ok(
                signature::EcdsaSecp384r1Verifier::from_sec1_bytes(&into_buffer_data(
                  inner.key.data,
                )?)
                .map_err(crate::Error::from)
                .map_err(napi::Error::from)?,
              ),
              VerifyingKeyFormat::SpkiDer => Ok(
                signature::EcdsaSecp384r1Verifier::from_public_key_der(&into_buffer_data(
                  inner.key.data,
                )?)
                .map_err(crate::Error::from)
                .map_err(napi::Error::from)?,
              ),
              VerifyingKeyFormat::SpkiPem => Ok(
                signature::EcdsaSecp384r1Verifier::from_public_key_pem(&into_string_data(
                  inner.key.data,
                )?)
                .map_err(crate::Error::from)
                .map_err(napi::Error::from)?,
              ),
              _ => Err(unsupported_key_format),
            }?;
            signature::SignatureVerifier::EcdsaSecp384r1(Arc::new(verifier))
          }
          SignatureAlgorithm::Ed25519 => {
            let verifier = match &inner.key.format {
              VerifyingKeyFormat::SpkiDer => Ok(
                signature::Ed25519Verifier::from_public_key_der(&into_buffer_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              VerifyingKeyFormat::SpkiPem => Ok(
                signature::Ed25519Verifier::from_public_key_pem(&into_string_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              VerifyingKeyFormat::Raw => {
                let data = into_buffer_data(inner.key.data)?;
                let bytes = data
                  .get(..32)
                  .and_then(|s| s.try_into().ok())
                  .ok_or_else(|| {
                    napi::Error::new(napi::Status::InvalidArg, "Expect 32 bytes for key pair")
                  })?;
                Ok(
                  signature::Ed25519Verifier::from_public_key_bytes(bytes)
                    .map_err(crate::Error::from)
                    .map_err(napi::Error::from)?,
                )
              }
              _ => Err(unsupported_key_format),
            }?;
            signature::SignatureVerifier::Ed25519(Arc::new(verifier))
          }
          SignatureAlgorithm::RsaPkcs1V1_5 => {
            let verifier = match &inner.key.format {
              VerifyingKeyFormat::Pkcs1Der => Ok(
                signature::RsaPkcs1V15Verifier::from_pkcs1_der(&into_buffer_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              VerifyingKeyFormat::Pkcs1Pem => Ok(
                signature::RsaPkcs1V15Verifier::from_pkcs1_pem(&into_string_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              VerifyingKeyFormat::SpkiDer => Ok(
                signature::RsaPkcs1V15Verifier::from_public_key_der(&into_buffer_data(
                  inner.key.data,
                )?)
                .map_err(crate::Error::from)
                .map_err(napi::Error::from)?,
              ),
              VerifyingKeyFormat::SpkiPem => Ok(
                signature::RsaPkcs1V15Verifier::from_public_key_pem(&into_string_data(
                  inner.key.data,
                )?)
                .map_err(crate::Error::from)
                .map_err(napi::Error::from)?,
              ),
              _ => Err(unsupported_key_format),
            }?;
            signature::SignatureVerifier::RsaPkcs1V15(Arc::new(verifier))
          }
          SignatureAlgorithm::RsaPss => {
            let verifier = match &inner.key.format {
              VerifyingKeyFormat::Pkcs1Der => Ok(
                signature::RsaPssVerifier::from_pkcs1_der(&into_buffer_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              VerifyingKeyFormat::Pkcs1Pem => Ok(
                signature::RsaPssVerifier::from_pkcs1_pem(&into_string_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              VerifyingKeyFormat::SpkiDer => Ok(
                signature::RsaPssVerifier::from_public_key_der(&into_buffer_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              VerifyingKeyFormat::SpkiPem => Ok(
                signature::RsaPssVerifier::from_public_key_pem(&into_string_data(inner.key.data)?)
                  .map_err(crate::Error::from)
                  .map_err(napi::Error::from)?,
              ),
              _ => Err(unsupported_key_format),
            }?;
            signature::SignatureVerifier::RsaPss(Arc::new(verifier))
          }
        },
        Either::B(inner) => {
          signature::SignatureVerifier::Custom(Arc::new(move |_bundle, message, signature| {
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

fn into_string_data(d: Either<String, Buffer>) -> napi::Result<String> {
  match d {
    Either::A(s) => Ok(s),
    Either::B(_) => Err(napi::Error::new(
      napi::Status::StringExpected,
      "Expect a string value",
    )),
  }
}

fn into_buffer_data(d: Either<String, Buffer>) -> napi::Result<Buffer> {
  match d {
    Either::A(_) => Err(napi::Error::new(
      napi::Status::ArrayBufferExpected,
      "Expect a array buffer value",
    )),
    Either::B(b) => Ok(b),
  }
}
