use crate::integrity::IntegrityPolicy;
use crate::remote::{ListRemoteBundleInfo, Remote, RemoteBundleInfo};
use crate::signature::SignatureVerifierOptions;
use crate::source::BundleSource;
use std::sync::Arc;
use wvb::updater;

/// Result of checking whether a bundle update is available.
///
/// `is_available` is `true` when `version` differs from `local_version`.
/// The `etag`, `integrity`, `signature`, and `last_modified` fields can be
/// passed to [`BundleSource::write_remote_bundle`] after downloading.
#[derive(uniffi::Record, Clone, Debug)]
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

impl From<updater::BundleUpdateInfo> for BundleUpdateInfo {
  fn from(value: updater::BundleUpdateInfo) -> Self {
    Self {
      name: value.name,
      version: value.version,
      local_version: value.local_version,
      is_available: value.is_available,
      etag: value.etag,
      integrity: value.integrity,
      signature: value.signature,
      last_modified: value.last_modified,
    }
  }
}

/// Optional configuration for the [`Updater`].
#[derive(uniffi::Record, Clone, Debug)]
pub struct UpdaterOptions {
  /// Release channel (e.g. `"stable"`, `"beta"`). Passed as a query parameter to the remote.
  pub channel: Option<String>,
  pub integrity_policy: Option<IntegrityPolicy>,
  /// When set, the updater verifies the bundle signature before applying an update.
  pub signature_verifier: Option<SignatureVerifierOptions>,
}

/// Orchestrates the full update cycle: checks for a new version on the remote,
/// downloads it, verifies integrity/signature, and writes it to the local source.
#[derive(uniffi::Object)]
pub struct Updater {
  inner: updater::Updater,
}

#[uniffi::export]
impl Updater {
  #[uniffi::constructor]
  pub fn new(
    source: Arc<BundleSource>,
    remote: Arc<Remote>,
    options: Option<UpdaterOptions>,
  ) -> Result<Arc<Updater>, crate::Error> {
    let config = if let Some(opts) = options {
      let mut config = updater::UpdaterConfig::default();
      if let Some(channel) = opts.channel {
        config = config.channel(channel);
      }
      if let Some(policy) = opts.integrity_policy {
        config = config.integrity_policy(policy.into());
      }
      if let Some(verifier_opts) = opts.signature_verifier {
        let verifier = wvb::signature::SignatureVerifier::try_from(verifier_opts)?;
        config = config.signature_verifier(verifier);
      }
      Some(config)
    } else {
      None
    };
    Ok(Arc::new(Updater {
      inner: updater::Updater::new(source.inner.clone(), remote.inner.clone(), config),
    }))
  }
}

#[uniffi::export(async_runtime = "tokio")]
impl Updater {
  pub async fn list_remotes(&self) -> Result<Vec<ListRemoteBundleInfo>, crate::Error> {
    let remotes = self
      .inner
      .list_remotes()
      .await?
      .into_iter()
      .map(ListRemoteBundleInfo::from)
      .collect();
    Ok(remotes)
  }

  /// Checks whether a newer version of `bundle_name` is available on the remote.
  /// Does not download the bundle.
  pub async fn get_update(&self, bundle_name: String) -> Result<BundleUpdateInfo, crate::Error> {
    let update = self.inner.get_update(&bundle_name).await?;
    Ok(BundleUpdateInfo::from(update))
  }

  /// Downloads and persists an update for `bundle_name`.
  /// Uses the latest remote version when `version` is `None`.
  pub async fn download_update(
    &self,
    bundle_name: String,
    version: Option<String>,
  ) -> Result<RemoteBundleInfo, crate::Error> {
    let info = self.inner.download(bundle_name, version).await?;
    Ok(info.into())
  }
}
