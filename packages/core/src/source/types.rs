#[cfg(feature = "integrity")]
use crate::integrity;
use crate::source::{ManifestBundleSet, ManifestVersionData};
use crate::{DataReadOptions, HeaderReadOptions, IndexReadOptions};

#[cfg(feature = "integrity")]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SourceIntegrityCheckMode {
  /// Verify both builtin and remote bundles.
  All,
  /// Check downloaded (remote) bundles only.
  #[default]
  OnlyRemote,
}

#[cfg(feature = "integrity")]
impl SourceIntegrityCheckMode {
  pub(crate) fn should_verify(&self, kind: &SourceKind) -> bool {
    match self {
      Self::All => true,
      Self::OnlyRemote => *kind == SourceKind::Remote,
    }
  }
}

/// How bundles are checked against the integrity recorded for them in the manifest when
/// they are loaded from disk.
#[cfg(feature = "integrity")]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SourceIntegrityOptions {
  pub policy: integrity::IntegrityPolicy,
  pub check_mode: SourceIntegrityCheckMode,
}

#[cfg(feature = "integrity")]
impl SourceIntegrityOptions {
  /// How a bundle's integrity metadata is treated
  pub fn policy(mut self, policy: integrity::IntegrityPolicy) -> Self {
    self.policy = policy;
    self
  }

  /// Which bundles are checked on load
  pub fn check_mode(mut self, mode: SourceIntegrityCheckMode) -> Self {
    self.check_mode = mode;
    self
  }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SourceOptions {
  pub header_read: HeaderReadOptions,
  pub index_read: IndexReadOptions,
  pub data_read: DataReadOptions,
  #[cfg(feature = "integrity")]
  pub integrity: SourceIntegrityOptions,
  pub remove_bundle_chunk_size: Option<usize>,
}

impl SourceOptions {
  pub fn header_read(mut self, options: HeaderReadOptions) -> Self {
    self.header_read = options;
    self
  }

  pub fn index_read(mut self, options: IndexReadOptions) -> Self {
    self.index_read = options;
    self
  }

  pub fn data_read(mut self, options: DataReadOptions) -> Self {
    self.data_read = options;
    self
  }

  #[cfg(feature = "integrity")]
  pub fn integrity(mut self, options: SourceIntegrityOptions) -> Self {
    self.integrity = options;
    self
  }

  pub fn remove_bundle_chunk_size(mut self, size: usize) -> Self {
    self.remove_bundle_chunk_size = Some(size);
    self
  }
}

/// The type of bundle source: builtin or remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
  /// Bundles shipped with the application (read-only, fallback)
  Builtin,
  /// Downloaded bundles (takes priority)
  Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestBundleItemStatus {
  Current,
  Previous,
  Staged,
  Orphan,
}

impl ManifestBundleItemStatus {
  pub(crate) fn from(bundle: &ManifestBundleSet, version: &str) -> Self {
    if let Some(current_version) = bundle.current_version.as_deref()
      && current_version == version
    {
      ManifestBundleItemStatus::Current
    } else if let Some(previous_version) = bundle.previous_version.as_deref()
      && previous_version == version
    {
      ManifestBundleItemStatus::Previous
    } else if let Some(staged_version) = bundle.staged_version.as_deref()
      && staged_version == version
    {
      ManifestBundleItemStatus::Staged
    } else {
      ManifestBundleItemStatus::Orphan
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestBundleItem {
  pub name: String,
  pub version: String,
  pub status: ManifestBundleItemStatus,
  pub data: ManifestVersionData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestSetCurrentVersionResultKind {
  Settled,
  /// The bundle was not exists in the manifest.
  NotExists,
  /// The version was not exists in the manifest.
  VersionNotExists,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestSetCurrentVersionResult {
  pub name: String,
  pub version: String,
  pub kind: ManifestSetCurrentVersionResultKind,
}

impl ManifestSetCurrentVersionResult {
  pub(crate) fn settled(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      kind: ManifestSetCurrentVersionResultKind::Settled,
    }
  }

  pub(crate) fn not_exists(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      kind: ManifestSetCurrentVersionResultKind::NotExists,
    }
  }

  pub(crate) fn version_not_exists(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      kind: ManifestSetCurrentVersionResultKind::VersionNotExists,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestStageData {
  pub version: String,
  pub data: Option<ManifestVersionData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestStageResultKind {
  Staged,
  /// The bundle is the current version so that can be not staged.
  InUse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestStageResult {
  pub name: String,
  pub version: String,
  pub kind: ManifestStageResultKind,
}

impl ManifestStageResult {
  pub(crate) fn staged(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      kind: ManifestStageResultKind::Staged,
    }
  }

  pub(crate) fn in_use(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      kind: ManifestStageResultKind::InUse,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestRemoveData {
  pub versions: Vec<String>,
  pub force: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestRemoveResultKind {
  Removed,
  /// The bundle was not exists in the manifest.
  NotExists,
  /// The version was not exists in the manifest.
  VersionNotExists,
  /// The bundle is the current version so that cant be not removed.
  /// This can be force by enable `force` option.
  InUse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestRemoveResult {
  pub name: String,
  pub version: String,
  pub kind: ManifestRemoveResultKind,
}

impl ManifestRemoveResult {
  pub(crate) fn removed(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      kind: ManifestRemoveResultKind::Removed,
    }
  }

  pub(crate) fn not_exists(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      kind: ManifestRemoveResultKind::NotExists,
    }
  }

  pub(crate) fn version_not_exists(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      kind: ManifestRemoveResultKind::VersionNotExists,
    }
  }

  pub(crate) fn in_use(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      kind: ManifestRemoveResultKind::InUse,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestPruneResult {
  pub name: String,
  pub pruned_versions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceListItem {
  pub source: SourceKind,
  pub item: ManifestBundleItem,
}

impl SourceListItem {
  pub(crate) fn from(source_kind: SourceKind, item: ManifestBundleItem) -> Self {
    Self {
      source: source_kind,
      item,
    }
  }
}
