use crate::remote::RemoteBundleInfo;
use crate::source::BundleManifestMetadata;

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

impl From<&RemoteBundleInfo> for BundleManifestMetadata {
  fn from(value: &RemoteBundleInfo) -> Self {
    Self {
      etag: value.etag.clone(),
      integrity: value.integrity.clone(),
      signature: value.signature.clone(),
      last_modified: value.last_modified.clone(),
    }
  }
}
