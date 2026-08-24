use crate::source::SourceKind;

/// Bundle version with source kind information.
///
/// This indicates which source (builtin or remote) provides a bundle version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleSourceVersion {
  /// The source kind (builtin or remote)
  pub source: SourceKind,
  /// The version string (e.g., "1.0.0")
  pub version: String,
}

impl BundleSourceVersion {
  /// Creates a new bundle source version.
  pub fn new(source: SourceKind, version: String) -> Self {
    Self { source, version }
  }

  /// Creates a builtin source version.
  pub fn builtin(version: String) -> Self {
    Self::new(SourceKind::Builtin, version)
  }

  /// Creates a remote source version.
  pub fn remote(version: String) -> Self {
    Self::new(SourceKind::Remote, version)
  }
}
