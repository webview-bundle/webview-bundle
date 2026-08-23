//! Digital signature verification.
//!
//! ## Features
//!
//! Enable specific signature algorithms via cargo features:
//!
//! - `signature-ecdsa-secp256r1` - ECDSA with P-256 curve
//! - `signature-ecdsa-secp384r1` - ECDSA with P-384 curve
//! - `signature-ed25519` - Ed25519 signatures
//! - `signature-rsa-pkcs1-v1_5-sha256` - RSA PKCS#1 v1.5 (sha256)
//! - `signature-rsa-pss-sha256` - RSA-PSS (sha256)
//!
//! ## Example
//!
//! Each algorithm implements [`SignatureVerifier`], and [`SignatureVerify`] collects them
//! into one enum implementing the same trait. Verification returns `Ok(())` when the
//! signature matches and an error otherwise, so the trait has to be in scope to call
//! [`SignatureVerifier::verify`].
//!
//! ```no_run
//! # #[cfg(feature = "signature-ed25519")]
//! # async {
//! use wvb::signature::{Ed25519, SignatureVerify, SignatureVerifier};
//!
//! // Create key with public key PEM
//! let public_key_pem = "-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----";
//! let key = SignatureVerify::Ed25519(Ed25519::from_public_key_pem(public_key_pem).unwrap());
//!
//! // The signed message is the bundle's integrity string, not the bundle bytes:
//! // the signature authenticates the integrity string, and the integrity string
//! // authenticates the bytes. Both checks must run for either to mean anything.
//! let message = b"sha256:n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg=";
//! let signature = "base64-encoded-signature";
//!
//! key.verify(message, signature).await.unwrap();
//! # };
//! ```
//!
//! ## Key Sets
//!
//! [`SignatureVerifyKey`] pairs a key with the id it is published under, and reports the
//! [`SignatureAlgorithm`] of the key it holds:
//!
//! ```no_run
//! # #[cfg(feature = "signature-ed25519")]
//! # {
//! use wvb::signature::{Ed25519, SignatureAlgorithm, SignatureVerify, SignatureVerifyKey};
//!
//! # let public_key_pem = "-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----";
//! let key_set = SignatureVerifyKey {
//!     id: "2026-08".to_string(),
//!     verify: SignatureVerify::Ed25519(Ed25519::from_public_key_pem(public_key_pem).unwrap()),
//! };
//! assert_eq!(key_set.algorithm(), SignatureAlgorithm::Ed25519);
//! # }
//! ```
//!
//! ## Custom Verifiers
//!
//! Implement custom verification logic:
//!
//! The closure is passed straight to [`SignatureVerify::Custom`] so its returned future
//! coerces to the boxed trait object the variant holds — binding it to a `let` first would
//! pin it to the concrete future type and fail to compile. Unlike the built-in algorithms
//! it reports its verdict as a `bool`; [`SignatureVerify`] turns `false` into
//! [`crate::Error::SignatureVerifyFailed`].
//!
//! ```no_run
//! # use wvb::signature::SignatureVerify;
//! # use std::sync::Arc;
//! let key = SignatureVerify::Custom(Arc::new(|message: &[u8], signature: &str| {
//!     let message = message.to_vec();
//!     let signature = signature.to_string();
//!     Box::pin(async move {
//!         // Custom verification logic
//!         let _ = (message, signature);
//!         Ok::<bool, Box<dyn std::error::Error + Send + Sync + 'static>>(true)
//!     })
//! }));
//! ```

mod alg;
#[cfg(feature = "signature-ecdsa-secp256r1")]
mod ecdsa_secp256r1;
#[cfg(feature = "signature-ecdsa-secp384r1")]
mod ecdsa_secp384r1;
#[cfg(feature = "signature-ed25519")]
mod ed25519;
#[cfg(feature = "signature-rsa-pkcs1-v1_5-sha256")]
mod rsa_pkcs1_v1_5_sha256;
#[cfg(feature = "signature-rsa-pss-sha256")]
mod rsa_pss_sha256;
mod verifier;
mod verify;

#[cfg(feature = "signature-ecdsa-secp256r1")]
pub use ecdsa_secp256r1::*;
#[cfg(feature = "signature-ecdsa-secp384r1")]
pub use ecdsa_secp384r1::*;
#[cfg(feature = "signature-ed25519")]
pub use ed25519::*;
#[cfg(feature = "signature-rsa-pkcs1-v1_5-sha256")]
pub use rsa_pkcs1_v1_5_sha256::*;
#[cfg(feature = "signature-rsa-pss-sha256")]
pub use rsa_pss_sha256::*;

pub use alg::*;
pub use verifier::*;
pub use verify::*;
