use std::collections::HashMap;

/// Representation of update info from the remote server.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct Update {
  /// Unique ID of this update. UUID format without hyphen('-').
  pub id: String,
  pub created_at: String,
  pub runtime_version: u8,
  pub directive: Option<UpdateDirective>,
  /// Bundle updates list
  pub bundles: Vec<BundleUpdate>,
  #[cfg_attr(feature = "_serde", serde(default))]
  pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub enum UpdateDirective {
  RollbackToBuiltin,
  CleanupAllRemotes,
  #[cfg_attr(feature = "_serde", serde(other))]
  Unknown,
}

/// Bundle update info
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct BundleUpdate {
  /// Name of bundle
  pub name: String,
  /// Version of bundle
  pub version: String,
  /// Update url
  /// If this not provided, default to fetch on `GET /bundles/:name/:version`.
  pub url: Option<String>,
  /// Integrity hash value
  pub integrity: Option<String>,
}
