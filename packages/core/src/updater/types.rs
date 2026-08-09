use crate::remote::BundleUpdate;
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
