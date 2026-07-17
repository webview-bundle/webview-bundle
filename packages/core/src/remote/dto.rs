/// Representation of bundle list info from the remote server.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct ListRemoteBundleInfo {
  /// Bundle name
  pub name: String,
  /// Version of the bundle
  pub version: String,
}

/// Representation of bundle info from the remote server.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct RemoteBundleInfo {
  /// Bundle name
  pub name: String,
  /// Version of the bundle
  pub version: String,
  /// ETag from the remote server. Can be used to check if the bundle has been updated.
  pub etag: Option<String>,
  /// Integrity hash of the bundle.
  pub integrity: Option<String>,
  /// Signature of the bundle.
  pub signature: Option<String>,
  /// Last modified date from the remote server.
  pub last_modified: Option<String>,
}

/// Error string representation for remote operations.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct RemoteError {
  /// Error message.
  pub message: Option<String>,
}
