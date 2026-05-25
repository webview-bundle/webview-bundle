#[cfg(feature = "integrity")]
use crate::integrity::{IntegrityChecker, IntegrityPolicy};
use crate::remote::{ListRemoteBundleInfo, Remote, RemoteBundleInfo};
#[cfg(feature = "signature")]
use crate::signature::SignatureVerifier;
use crate::source::{BundleManifestMetadata, BundleSource};
use std::sync::Arc;

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

#[derive(Default)]
#[non_exhaustive]
pub struct UpdaterConfig {
  pub(crate) channel: Option<String>,
  #[cfg(feature = "integrity")]
  pub(crate) integrity_checker: IntegrityChecker,
  #[cfg(feature = "integrity")]
  pub(crate) integrity_policy: IntegrityPolicy,
  #[cfg(feature = "signature")]
  pub(crate) signature_verifier: Option<SignatureVerifier>,
}

impl UpdaterConfig {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn channel(mut self, channel: impl Into<String>) -> Self {
    self.channel = Some(channel.into());
    self
  }

  #[cfg(feature = "integrity")]
  pub fn integrity_checker(mut self, checker: IntegrityChecker) -> Self {
    self.integrity_checker = checker;
    self
  }

  #[cfg(feature = "integrity")]
  pub fn integrity_policy(mut self, policy: IntegrityPolicy) -> Self {
    self.integrity_policy = policy;
    self
  }

  #[cfg(feature = "signature")]
  pub fn signature_verifier(mut self, verifier: SignatureVerifier) -> Self {
    self.signature_verifier = Some(verifier);
    self
  }
}

pub struct Updater {
  source: Arc<BundleSource>,
  remote: Arc<Remote>,
  config: UpdaterConfig,
}

impl Updater {
  pub fn new(
    source: Arc<BundleSource>,
    remote: Arc<Remote>,
    config: Option<UpdaterConfig>,
  ) -> Self {
    Self {
      source,
      remote,
      config: config.unwrap_or_default(),
    }
  }

  pub async fn list_remotes(&self) -> crate::Result<Vec<ListRemoteBundleInfo>> {
    self.remote.list_bundles(self.config.channel.as_ref()).await
  }

  pub async fn get_update(
    &self,
    bundle_name: impl Into<String>,
  ) -> crate::Result<BundleUpdateInfo> {
    let remote_info = self
      .remote
      .get_current_info(&bundle_name.into(), self.config.channel.as_ref())
      .await?;
    let info = self.to_update_info(remote_info).await?;
    Ok(info)
  }

  pub async fn download_update(
    &self,
    bundle_name: impl Into<String>,
    version: Option<String>,
  ) -> crate::Result<RemoteBundleInfo> {
    let bundle_name = bundle_name.into();
    // Serialize against concurrent installs/downloads of this same bundle.
    let _guard = self.source.lock_bundle(&bundle_name).await;
    let (info, bundle, data) = match version {
      Some(ver) => self.remote.download_version(&bundle_name, &ver).await,
      None => {
        self
          .remote
          .download(&bundle_name, self.config.channel.as_ref())
          .await
      }
    }?;
    #[cfg(feature = "integrity")]
    self
      .verify_bundle(
        info.integrity.as_deref(),
        info.signature.as_deref(),
        &bundle,
        &data,
      )
      .await?;
    // `data` (raw bytes) is only consumed by the integrity check.
    #[cfg(not(feature = "integrity"))]
    let _ = &data;
    self
      .source
      .write_remote_bundle(
        &info.name,
        &info.version,
        &bundle,
        BundleManifestMetadata::from(&info),
      )
      .await?;
    Ok(info)
  }

  /// Activates a previously-downloaded version so the protocol begins serving it.
  ///
  /// The version must already be staged in the remote manifest (downloaded). Steps:
  /// 1. confirm the version is staged (else error);
  /// 2. validate the on-disk bundle — structural parse always, plus integrity/signature
  ///    against the metadata captured at download time (when those features are enabled);
  /// 3. record it as current (demoting the old current to previous) and persist;
  /// 4. drop the cached descriptor so the next request observes the new version;
  /// 5. prune older staged versions, keeping {current, previous} (best-effort).
  ///
  /// On any failure before step 3 the active version is left unchanged and the
  /// downloaded files are kept, so a retry (or rollback to a still-present version)
  /// remains possible.
  pub async fn install(
    &self,
    bundle_name: impl Into<String>,
    version: impl Into<String>,
  ) -> crate::Result<()> {
    let bundle_name = bundle_name.into();
    let version = version.into();

    // Serialize the whole transaction against other installs/downloads of this bundle.
    let _guard = self.source.lock_bundle(&bundle_name).await;

    // 1. The version must be staged in the remote manifest.
    let metadata = self
      .source
      .load_remote_metadata(&bundle_name, &version)
      .await?
      .ok_or_else(|| crate::Error::bundle_entry_not_exists(&bundle_name, &version))?;

    // 2. Validate the on-disk bundle. Parsing alone enforces structural validity
    //    (magic/checksum/truncation); integrity & signature add semantic verification.
    let (_bundle, _data) = self
      .source
      .read_remote_version(&bundle_name, &version)
      .await?;
    #[cfg(feature = "integrity")]
    self
      .verify_bundle(
        metadata.integrity.as_deref(),
        metadata.signature.as_deref(),
        &_bundle,
        &_data,
      )
      .await?;
    #[cfg(not(feature = "integrity"))]
    let _ = &metadata;

    // 3. Activate (records current/previous, persists atomically).
    self.source.update_version(&bundle_name, &version).await?;

    // 4. Next protocol request rebuilds the descriptor from the new version.
    self.source.unload_descriptor(&bundle_name);

    // 5. Reclaim disk, keeping the active and immediately-previous versions.
    let _ = self.source.prune_remote_bundles(&bundle_name).await;

    Ok(())
  }

  /// Verifies a bundle against its advertised integrity/signature, per the configured
  /// policy. Shared by the download (fresh bytes) and install (on-disk bytes) paths.
  #[cfg(feature = "integrity")]
  async fn verify_bundle(
    &self,
    integrity: Option<&str>,
    signature: Option<&str>,
    bundle: &crate::Bundle,
    data: &[u8],
  ) -> crate::Result<()> {
    match self.config.integrity_policy {
      IntegrityPolicy::Strict | IntegrityPolicy::Optional => {
        if let Some(integrity) = integrity {
          self.config.integrity_checker.check(integrity, data).await?;
        } else if self.config.integrity_policy == IntegrityPolicy::Strict {
          return Err(crate::Error::IntegrityVerifyFailed);
        }
      }
      _ => {}
    }
    #[cfg(feature = "signature")]
    if let Some(ref verifier) = self.config.signature_verifier {
      let message = integrity.ok_or(crate::Error::IntegrityRequired)?;
      let signature = signature.ok_or(crate::Error::SignatureNotExists)?;
      if !verifier
        .verify(bundle, message.as_bytes(), signature)
        .await?
      {
        return Err(crate::Error::SignatureVerifyFailed);
      }
    }
    Ok(())
  }

  async fn to_update_info(&self, info: RemoteBundleInfo) -> crate::Result<BundleUpdateInfo> {
    let local_version = self.source.load_version(&info.name).await?;
    let is_available = if let Some(ref local_ver) = local_version {
      local_ver.version != info.version
    } else {
      true
    };
    Ok(BundleUpdateInfo {
      name: info.name,
      version: info.version,
      local_version: local_version.map(|x| x.version),
      is_available,
      etag: info.etag.clone(),
      integrity: info.integrity.clone(),
      signature: info.signature.clone(),
      last_modified: info.last_modified.clone(),
    })
  }
}
