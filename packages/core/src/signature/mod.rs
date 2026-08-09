//! Digital signature verification for bundles.
//!
//! The signature module provides cryptographic signature verification to ensure
//! bundles are authentic and haven't been tampered with. Multiple signature
//! algorithms are supported:
//!
//! - **ECDSA** (secp256r1, secp384r1) - Elliptic curve signatures
//! - **Ed25519** - Edwards curve signatures
//! - **RSA** (PKCS#1 v1.5, PSS) - RSA signatures
//!
//! ## Features
//!
//! Enable specific signature algorithms via cargo features:
//!
//! - `signature-ecdsa_secp256r1` - ECDSA with P-256 curve
//! - `signature-ecdsa_secp384r1` - ECDSA with P-384 curve
//! - `signature-edd25519` - Ed25519 signatures
//! - `signature-rsa_pkcs1_v1_5` - RSA PKCS#1 v1.5
//! - `signature-rsa_pss` - RSA-PSS
//!
//! ## Example
//!
//! ```no_run
//! # #[cfg(feature = "signature-edd25519")]
//! # async {
//! use wvb::signature::{Ed25519, SignatureVerify};
//! use std::sync::Arc;
//!
//! // Create verifier with public key PEM
//! let public_key_pem = "-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----";
//! let verifier = Ed25519::from_public_key_pem(public_key_pem).unwrap();
//! let signature_verifier = SignatureVerify::Ed25519(Arc::new(verifier));
//!
//! // The signed message is the bundle's integrity string, not the bundle bytes:
//! // the signature authenticates the integrity string, and the integrity string
//! // authenticates the bytes. Both checks must run for either to mean anything.
//! let message = b"sha256:n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg=";
//! let signature = "base64-encoded-signature";
//!
//! let is_valid = signature_verifier.verify(message, signature).await.unwrap();
//! assert!(is_valid);
//! # };
//! ```
//!
//! ## Custom Verifiers
//!
//! Implement custom verification logic:
//!
//! The closure is passed straight to [`SignatureVerify::Custom`] so its returned future
//! coerces to the boxed trait object the variant holds — binding it to a `let` first would
//! pin it to the concrete future type and fail to compile.
//!
//! ```no_run
//! # use wvb::signature::SignatureVerify;
//! # use std::sync::Arc;
//! let verifier = SignatureVerify::Custom(Arc::new(|message: &[u8], signature: &str| {
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
mod key;
#[cfg(feature = "signature-rsa-pkcs1-v1_5-sha256")]
mod rsa_pkcs1_v1_5_sha256;
#[cfg(feature = "signature-rsa-pss-sha256")]
mod rsa_pss_sha256;
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
pub use key::*;
pub use verify::*;
