#[cfg(feature = "integrity")]
use crate::integrity;
use crate::{DataReadOptions, HeaderReadOptions, IndexReadOptions};
use std::collections::HashMap;

/// Which bundles a load-time integrity verification applies to.
#[cfg(any(feature = "integrity", feature = "signature"))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BundleSourceVerifyMode {
  /// Verify both builtin and remote bundles.
  All,
  /// Verify downloaded (remote) bundles only.
  #[default]
  OnlyRemote,
}

#[cfg(any(feature = "integrity", feature = "signature"))]
impl BundleSourceVerifyMode {
  pub(crate) fn should_verify(&self, kind: &BundleSourceKind) -> bool {
    match self {
      Self::All => true,
      Self::OnlyRemote => *kind == BundleSourceKind::Remote,
    }
  }
}

/// How bundles are checked against the integrity recorded for them in the manifest when
/// they are loaded from disk.
#[cfg(feature = "integrity")]
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct BundleSourceIntegrityOptions {
  pub policy: integrity::IntegrityPolicy,
  pub check_mode: BundleSourceVerifyMode,
}

#[cfg(feature = "integrity")]
impl BundleSourceIntegrityOptions {
  /// How a bundle's integrity metadata is treated
  pub fn policy(mut self, policy: integrity::IntegrityPolicy) -> Self {
    self.policy = policy;
    self
  }

  /// Which bundles are checked on load
  pub fn check_mode(mut self, mode: BundleSourceVerifyMode) -> Self {
    self.check_mode = mode;
    self
  }
}

#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct BundleSourceOptions {
  pub header_read: HeaderReadOptions,
  pub index_read: IndexReadOptions,
  pub data_read: DataReadOptions,
  #[cfg(feature = "integrity")]
  pub integrity: BundleSourceIntegrityOptions,
}

impl BundleSourceOptions {
  /// How a bundle's header is checked when its descriptor is read on load.
  pub fn header_read(mut self, options: HeaderReadOptions) -> Self {
    self.header_read = options;
    self
  }

  /// How a bundle's index is checked when its descriptor is read on load.
  pub fn index_read(mut self, options: IndexReadOptions) -> Self {
    self.index_read = options;
    self
  }

  /// How entry data read through this source is checked
  pub fn data_read(mut self, options: DataReadOptions) -> Self {
    self.data_read = options;
    self
  }

  /// How bundles are checked against their manifest integrity metadata on load.
  #[cfg(feature = "integrity")]
  pub fn integrity(mut self, options: BundleSourceIntegrityOptions) -> Self {
    self.integrity = options;
    self
  }
}

/// The type of bundle source: builtin or remote.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "snake_case"))]
pub enum BundleSourceKind {
  /// Bundles shipped with the application (read-only, fallback)
  Builtin,
  /// Downloaded bundles (takes priority)
  Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
  feature = "_serde",
  derive(serde_repr::Serialize_repr, serde_repr::Deserialize_repr)
)]
#[cfg_attr(feature = "_serde", repr(u8))]
pub enum BundleManifestVersion {
  #[default]
  V1 = 1,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct BundleManifestVersionData {
  pub integrity: Option<String>,
  pub metadata: Option<HashMap<String, String>>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct BundleManifestEntry {
  pub versions: HashMap<String, BundleManifestVersionData>,
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
#[non_exhaustive]
pub struct BundleManifestData {
  pub manifest_version: BundleManifestVersion,
  pub entries: HashMap<String, BundleManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "snake_case"))]
pub enum BundleManifestEntryItemStatus {
  Current,
  Previous,
  Staged,
  Orphan,
}

impl BundleManifestEntryItemStatus {
  pub(crate) fn from(entry: &BundleManifestEntry, version: &str) -> Self {
    if let Some(current_version) = entry.current_version.as_deref()
      && current_version == version
    {
      BundleManifestEntryItemStatus::Current
    } else if let Some(previous_version) = entry.previous_version.as_deref()
      && previous_version == version
    {
      BundleManifestEntryItemStatus::Previous
    } else if let Some(staged_version) = entry.staged_version.as_deref()
      && staged_version == version
    {
      BundleManifestEntryItemStatus::Staged
    } else {
      BundleManifestEntryItemStatus::Orphan
    }
  }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub struct BundleManifestEntryItem {
  pub name: String,
  pub version: String,
  pub status: BundleManifestEntryItemStatus,
  pub data: BundleManifestVersionData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "snake_case"))]
pub enum BundleEntryRemoveResultKind {
  Removed,
  /// The entry was not in the manifest.
  NotFound,
  /// The entry is the current version and `force` option was not set.
  InUse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct BundleEntryRemoveResult {
  pub name: String,
  pub version: String,
  #[cfg_attr(feature = "_serde", serde(rename = "type"))]
  pub kind: BundleEntryRemoveResultKind,
}

impl BundleEntryRemoveResult {
  pub(crate) fn removed(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      kind: BundleEntryRemoveResultKind::Removed,
    }
  }

  pub(crate) fn not_found(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      kind: BundleEntryRemoveResultKind::NotFound,
    }
  }

  pub(crate) fn in_use(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      kind: BundleEntryRemoveResultKind::InUse,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct BundleEntryPruneResult {
  pub name: String,
  pub pruned_versions: Vec<String>,
}
