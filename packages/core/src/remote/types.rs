use crate::remote::sfv::parse_string_dict;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
/// Options used when requesting the current update document.
pub struct RemoteGetUpdateOptions {
  /// Optional etag value which got from previous update.
  /// If this value is provided, it is used for the "if-none-match" header value.
  pub etag: Option<String>,
  /// Channel of this update.
  pub channel: Option<String>,
  #[cfg(feature = "signature")]
  /// The client requests the signature information to be used for verification.
  pub expect_signature: Option<crate::signature::SignatureVerifyKey>,
}

impl RemoteGetUpdateOptions {
  /// Sends `etag` as the `If-None-Match` request header.
  ///
  /// A matching remote response is represented by `Ok(None)` from
  /// [`Remote::get_update`](crate::remote::Remote::get_update).
  pub fn etag(mut self, etag: impl Into<String>) -> Self {
    self.etag = Some(etag.into());
    self
  }

  /// Requests the update published for `channel`.
  pub fn channel(mut self, channel: impl Into<String>) -> Self {
    self.channel = Some(channel.into());
    self
  }

  #[cfg(feature = "signature")]
  /// Requires the response to be signed by `sig`.
  ///
  /// The request advertises the key id and algorithm to the remote server; the response body is
  /// verified before it is parsed.
  pub fn expect_signature(mut self, sig: crate::signature::SignatureVerifyKey) -> Self {
    self.expect_signature = Some(sig);
    self
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
/// A successful response from [`Remote::get_update`](crate::remote::Remote::get_update).
pub struct RemoteUpdateResponse {
  /// Update information which parsed from response body.
  pub update: Update,
  /// "etag" value received from server response.
  pub etag: Option<String>,
  /// Signature information for this update.
  /// Client can verify the signature using the public key.
  pub signature: Option<UpdateSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
/// A complete, atomically published set of bundle updates.
pub struct Update {
  /// The unique id of this update.
  pub id: String,
  /// The date and time at which the update was created.
  /// The datetime should be formatted according to [ISO 8601].
  ///
  /// [ISO 8601]: https://en.wikipedia.org/wiki/ISO_8601
  pub created_at: String,
  /// This is managed internally by the library and uses a versioning scheme
  /// distinct from the package version; it is utilized to ensure version compatibility
  /// within the update model.
  pub runtime_version: u8,
  /// An array of bundle updates.
  pub bundles: Vec<BundleUpdate>,
  /// The metadata associated with an update.
  /// Metadata should be a string-valued dictionary.
  pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
/// One bundle contained in an [`Update`].
pub struct BundleUpdate {
  /// Name of the bundle.
  pub name: String,
  /// Version of the bundle.
  pub version: String,
  /// Optional download url which server defined.
  /// If this not specified, client will download bundle with default url:
  /// `GET /bundles/:name/:version` (`base_url` is used from the remote)
  pub download_url: Option<String>,
  /// Hash of the file to guarantee integrity.
  pub integrity: Option<String>,
  /// Provider-defined, string-valued metadata for this bundle.
  pub metadata: Option<HashMap<String, String>>,
}

/// Bundle download signature info
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct UpdateSignature {
  /// Identifier of the verification key that produced the signature.
  pub key_id: String,
  /// Base64-encoded signature of the raw update response body.
  pub sig: String,
  /// Signature algorithm used for [`Self::sig`].
  pub alg: String,
}

/// Parses the `wvb-signature` header, which is a [RFC 8941] dictionary.
///
/// [RFC 8941]: https://www.rfc-editor.org/rfc/rfc8941#name-dictionaries
impl FromStr for UpdateSignature {
  type Err = crate::Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let mut dict = parse_string_dict(s).ok_or_else(|| {
      crate::Error::bad_remote_response(format!("\"wvb-signature\" header is malformed: {s:?}"))
    })?;
    let key_id = dict.remove("key_id").ok_or_else(|| {
      crate::Error::bad_remote_response(
        "\"wvb-signature\" header is missing \"key_id\"".to_string(),
      )
    })?;
    let alg = dict.remove("alg").ok_or_else(|| {
      crate::Error::bad_remote_response("\"wvb-signature\" header is missing \"alg\"")
    })?;
    let sig = dict.remove("sig").ok_or_else(|| {
      crate::Error::bad_remote_response("\"wvb-signature\" header is missing \"sig\"")
    })?;
    Ok(Self { key_id, sig, alg })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_header() {
    assert_eq!(
      r#"key_id="somekey", alg="alg", sig="value""#.parse::<UpdateSignature>().unwrap(),
      UpdateSignature {
        key_id: "somekey".to_string(),
        sig: "value".to_owned(),
        alg: "alg".to_owned(),
      }
    );
  }

  #[test]
  fn rejects_header_with_missing_member() {
    let err = r#"key_id="somekey", alg="alg""#.parse::<UpdateSignature>().unwrap_err();
    assert!(matches!(err, crate::Error::BadRemoteResponse(_)));

    let err = r#"key_id="somekey", sig="value""#.parse::<UpdateSignature>().unwrap_err();
    assert!(matches!(err, crate::Error::BadRemoteResponse(_)));

    let err = r#"alg="alg", sig="value""#.parse::<UpdateSignature>().unwrap_err();
    assert!(matches!(err, crate::Error::BadRemoteResponse(_)));
  }

  #[test]
  fn rejects_malformed_header() {
    let err = r#"alg=alg, sig=value"#.parse::<UpdateSignature>().unwrap_err();
    assert!(matches!(err, crate::Error::BadRemoteResponse(_)));

    let err = r#"keyId="somekey", alg="alg", sig="value""#.parse::<UpdateSignature>().unwrap_err();
    assert!(matches!(err, crate::Error::BadRemoteResponse(_)));
  }
}

/// Error string representation for remote operations.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct RemoteError {
  /// Error message.
  pub message: Option<String>,
}
