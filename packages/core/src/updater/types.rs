use crate::remote::{BundleUpdate, RemoteBundleInfo};
use crate::source::BundleManifestVersionData;
use crate::util::cancellation::Cancellation;

#[derive(Debug, Clone)]
pub struct GetUpdateOptions {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadBundle {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DownloadOptions {
  pub concurrency: Option<usize>,
  pub timeout: Option<u64>,
  pub cancellation: Option<Cancellation>,
}

#[derive(Debug)]
pub struct DownloadResult {
  pub update: BundleUpdate,
  pub result: crate::Result<()>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallBundleTarget {
  pub name: String,
  pub version: String,
  /// Bundles that must be installed together with this one. Targets are grouped transitively,
  /// so declaring the relation on one side is enough.
  pub atomic: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct InstallResult {
  pub target: InstallBundleTarget,
  pub result: crate::Result<()>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "_serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "_serde", serde(rename_all = "camelCase"))]
pub struct BundleUpdateInfo {
  pub name: String,
  pub version: String,
  pub local_version: Option<String>,
  pub is_available: bool,
  pub etag: Option<String>,
  pub integrity: Option<String>,
  pub signature: Option<String>,
  pub last_modified: Option<String>,
}

impl From<&BundleUpdateInfo> for RemoteBundleInfo {
  fn from(value: &BundleUpdateInfo) -> Self {
    Self {
      name: value.name.to_string(),
      version: value.version.to_string(),
      etag: value.etag.clone(),
      integrity: value.integrity.clone(),
      signature: value.signature.clone(),
      last_modified: value.last_modified.clone(),
    }
  }
}

impl From<&RemoteBundleInfo> for BundleManifestVersionData {
  fn from(value: &RemoteBundleInfo) -> Self {
    Self {
      etag: value.etag.clone(),
      integrity: value.integrity.clone(),
      signature: value.signature.clone(),
      last_modified: value.last_modified.clone(),
    }
  }
}
