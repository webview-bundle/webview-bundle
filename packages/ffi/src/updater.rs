use crate::integrity::{IntegrityCheck, IntegrityPolicy};
use crate::remote::{ListRemoteBundleInfo, Remote, RemoteBundleInfo};
use crate::signature::SignatureVerification;
use crate::source::BundleSource;
use std::sync::Arc;
use wvb::signature;
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

#[derive(uniffi::Record, Clone)]
pub struct UpdaterIntegrityOptions {
  /// How a bundle's integrity metadata is treated (default: [`IntegrityPolicy::Optional`]).
  ///
  /// [`IntegrityPolicy::Off`] disables the integrity check entirely.
  #[uniffi(default = None)]
  pub policy: Option<IntegrityPolicy>,
  /// A custom checker that validates bundle bytes against their integrity string
  /// (default: the built-in checker, which compares the advertised hash).
  #[uniffi(default = None)]
  pub check: Option<Arc<dyn IntegrityCheck>>,
}

#[derive(uniffi::Record, Clone)]
pub struct UpdaterSignatureOptions {
  /// When set, the updater verifies the bundle signature over its integrity string before
  /// applying an update — with a declarative public key or a custom function.
  /// Verified independently of `integrity_policy` — keep the policy enabled for the
  /// signature to also authenticate the bundle bytes.
  #[uniffi(default = None)]
  pub verify: Option<SignatureVerification>,
}

/// Optional configuration for the [`Updater`].
#[derive(uniffi::Record, Clone)]
pub struct UpdaterOptions {
  /// Release channel (e.g. `"stable"`, `"beta"`). Passed as a query parameter to the remote.
  #[uniffi(default = None)]
  pub channel: Option<String>,
  #[uniffi(default = None)]
  pub integrity: Option<UpdaterIntegrityOptions>,
  #[uniffi(default = None)]
  pub signature: Option<UpdaterSignatureOptions>,
}

fn updater_options(options: UpdaterOptions) -> Result<updater::UpdaterOptions, crate::Error> {
  let mut updater_options = updater::UpdaterOptions::default();

  if let Some(channel) = options.channel {
    updater_options = updater_options.channel(channel);
  }

  if let Some(integrity) = options.integrity {
    let mut integrity_options = updater::UpdaterIntegrityOptions::default();
    if let Some(policy) = integrity.policy {
      integrity_options = integrity_options.policy(policy.into());
    }
    if let Some(check) = integrity.check {
      integrity_options = integrity_options.algorithm(crate::integrity::into_checker(check));
    }
    updater_options = updater_options.integrity(integrity_options);
  }

  if let Some(signature) = options.signature {
    let mut signature_options = updater::UpdaterSignatureOptions::default();
    if let Some(verify) = signature.verify {
      signature_options = signature_options.verify(signature::SignatureVerify::try_from(verify)?);
    }
    updater_options = updater_options.signature(signature_options);
  }

  Ok(updater_options)
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
    let config = match options {
      Some(options) => Some(updater_options(options)?),
      None => None,
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

  /// Activates a previously downloaded bundle version.
  ///
  /// The version must already be staged in the remote source (via
  /// [`download_update`](Updater::download_update)). When integrity/signature
  /// verification is configured, the staged bundle is verified before activation.
  /// On success the current version is updated, the cached descriptor is dropped,
  /// and stale staged versions are pruned.
  pub async fn install(&self, bundle_name: String, version: String) -> Result<(), crate::Error> {
    self.inner.install(bundle_name, version).await?;
    Ok(())
  }
}
