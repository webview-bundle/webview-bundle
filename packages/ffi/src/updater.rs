use crate::integrity::IntegrityPolicy;
use crate::remote::{ListRemoteBundleInfo, Remote, RemoteBundleInfo};
use crate::signature::SignatureVerifierOptions;
use crate::source::BundleSource;
use std::sync::Arc;
use wvb::updater;

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

#[derive(uniffi::Record, Clone, Debug)]
pub struct UpdaterOptions {
  pub channel: Option<String>,
  pub integrity_policy: Option<IntegrityPolicy>,
  pub signature_verifier: Option<SignatureVerifierOptions>,
}

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

  pub async fn get_update(&self, bundle_name: String) -> Result<BundleUpdateInfo, crate::Error> {
    let update = self.inner.get_update(&bundle_name).await?;
    Ok(BundleUpdateInfo::from(update))
  }

  pub async fn download_update(
    &self,
    bundle_name: String,
    version: Option<String>,
  ) -> Result<RemoteBundleInfo, crate::Error> {
    let info = self.inner.download_update(bundle_name, version).await?;
    Ok(info.into())
  }
}
