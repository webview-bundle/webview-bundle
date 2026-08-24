use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
  feature = "_serde",
  derive(serde_repr::Serialize_repr, serde_repr::Deserialize_repr)
)]
#[cfg_attr(feature = "_serde", repr(u8))]
pub enum ManifestVersion {
  #[default]
  V1 = 1,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct ManifestVersionData {
  pub integrity: Option<String>,
  pub metadata: Option<HashMap<String, String>>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct ManifestBundleSet {
  pub versions: HashMap<String, ManifestVersionData>,
  /// The current version, or `None` when versions are present on disk but none has
  /// been activated yet.
  #[cfg_attr(
    feature = "_serde",
    serde(default, skip_serializing_if = "Option::is_none")
  )]
  pub current_version: Option<String>,
  /// The previous version that was recorded before the current version changed.
  #[cfg_attr(
    feature = "_serde",
    serde(default, skip_serializing_if = "Option::is_none")
  )]
  pub previous_version: Option<String>,
  /// The staged version that has been downloaded from remote.
  #[cfg_attr(
    feature = "_serde",
    serde(default, skip_serializing_if = "Option::is_none")
  )]
  pub staged_version: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct ManifestData {
  pub manifest_version: ManifestVersion,
  pub bundles: HashMap<String, ManifestBundleSet>,
}
