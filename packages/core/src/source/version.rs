use crate::source::types::BundleSourceKind;

/// Bundle version with source kind information.
///
/// This indicates which source (builtin or remote) provides a bundle version.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct BundleSourceVersion {
  /// The source kind (builtin or remote)
  #[cfg_attr(feature = "_serde", serde(rename = "type"))]
  pub kind: BundleSourceKind,
  /// The version string (e.g., "1.0.0")
  pub version: String,
}

impl BundleSourceVersion {
  /// Creates a new bundle source version.
  pub fn new(kind: BundleSourceKind, version: String) -> Self {
    Self { kind, version }
  }

  /// Creates a builtin source version.
  pub fn builtin(version: String) -> Self {
    Self::new(BundleSourceKind::Builtin, version)
  }

  /// Creates a remote source version.
  pub fn remote(version: String) -> Self {
    Self::new(BundleSourceKind::Remote, version)
  }
}
