use crate::remote::{ListRemoteBundleInfo, Remote, RemoteBundleInfo, RemoteFetchOptions};
use crate::source::{BundleManifestMetadata, BundleSource};
use crate::updater::{BundleUpdateInfo, UpdaterOptions};
use crate::{BundleDescriptor, BundleReader, Reader};
use dashmap::DashMap;
use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard};

pub struct Updater {
  source: Arc<BundleSource>,
  remote: Arc<Remote>,
  options: UpdaterOptions,
  // Per-bundle transaction lock.
  locks: DashMap<String, Arc<Mutex<()>>>,
}

impl Updater {
  pub fn new(
    source: Arc<BundleSource>,
    remote: Arc<Remote>,
    options: Option<UpdaterOptions>,
  ) -> Self {
    Self {
      source,
      remote,
      options: options.unwrap_or_default(),
      locks: DashMap::new(),
    }
  }

  pub async fn list_remotes(&self) -> crate::Result<Vec<ListRemoteBundleInfo>> {
    self
      .remote
      .list_bundles(Some(self.to_fetch_options()))
      .await
  }

  pub async fn get_update(
    &self,
    bundle_name: impl Into<String>,
  ) -> crate::Result<BundleUpdateInfo> {
    let remote_info = self
      .remote
      .get_current_info(&bundle_name.into(), Some(self.to_fetch_options()))
      .await?;
    let info = self.to_update_info(remote_info).await?;
    Ok(info)
  }

  /// Download a new version of a bundle and save into disk.
  ///
  /// Downloaded bundle does not activate automatically.
  /// To activate the downloaded version, use `Updater::install`.
  pub async fn download(
    &self,
    bundle_name: impl Into<String>,
    version: Option<String>,
  ) -> crate::Result<RemoteBundleInfo> {
    let bundle_name = bundle_name.into();
    let _guard = self.lock_bundle(&bundle_name).await;
    let (info, _bundle, data) = match version {
      Some(ver) => self.remote.download_version(&bundle_name, &ver).await,
      None => {
        self
          .remote
          .download(&bundle_name, self.options.channel.as_ref())
          .await
      }
    }?;

    #[cfg(feature = "integrity")]
    self
      .verify(&data, info.integrity.as_deref(), info.signature.as_deref())
      .await?;

    self
      .source
      .write_remote_bundle_data(
        &info.name,
        &info.version,
        &data,
        BundleManifestMetadata::from(&info),
      )
      .await?;
    Ok(info)
  }

  /// Activates a downloaded version so the protocol begins serving it.
  ///
  /// The version must already be staged in the remote manifest (downloaded).
  pub async fn install(
    &self,
    bundle_name: impl Into<String>,
    version: impl Into<String>,
  ) -> crate::Result<()> {
    let bundle_name = bundle_name.into();
    let version = version.into();

    let _guard = self.lock_bundle(&bundle_name).await;

    let metadata = self
      .source
      .load_remote_metadata(&bundle_name, &version)
      .await?
      .ok_or_else(|| crate::Error::bundle_entry_not_exists(&bundle_name, &version))?;

    let filepath = self
      .source
      .get_remote_bundle_filepath(&bundle_name, &version)?;
    let data = tokio::fs::read(&filepath).await?;

    // Parse the staged file before activating it.
    Reader::<BundleDescriptor>::read(&mut BundleReader::new(Cursor::new(&data)))?;

    #[cfg(feature = "integrity")]
    self
      .verify(
        &data,
        metadata.integrity.as_deref(),
        metadata.signature.as_deref(),
      )
      .await?;

    #[cfg(not(feature = "integrity"))]
    let _ = &metadata;

    self
      .source
      .update_remote_version(&bundle_name, &version)
      .await?;
    self.source.unload_descriptor(&bundle_name);
    let _ = self.source.prune_remote_bundles(&bundle_name).await;

    Ok(())
  }

  fn to_fetch_options(&self) -> RemoteFetchOptions {
    let mut options = RemoteFetchOptions::default();
    if let Some(channel) = &self.options.channel {
      options = options.channel(channel);
    }
    options
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

  /// Acquires the per-bundle transaction lock so concurrent update transactions for the
  /// same bundle run serially. Held for the whole of `download_update` / `install`.
  async fn lock_bundle(&self, bundle_name: &str) -> OwnedMutexGuard<()> {
    let mutex = self
      .locks
      .entry(bundle_name.to_string())
      .or_default()
      .clone();
    mutex.lock_owned().await
  }

  async fn verify(
    &self,
    data: &[u8],
    integrity: Option<&str>,
    signature: Option<&str>,
  ) -> crate::Result<()> {
    #[cfg(feature = "integrity")]
    {
      crate::integrity::verify_integrity(
        &self.options.integrity.policy,
        &self.options.integrity.check,
        integrity,
        data,
      )
      .await?;
    }
    #[cfg(feature = "signature")]
    {
      if let Some(verify) = &self.options.signature.verify {
        crate::signature::verify_signature(verify, integrity, signature).await?;
      }
    }
    #[cfg(not(feature = "integrity"))]
    {
      let _ = (data, metadata);
    }
    Ok(())
  }
}
