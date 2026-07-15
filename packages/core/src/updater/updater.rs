#[cfg(feature = "integrity")]
use crate::integrity::{IntegrityChecker, IntegrityPolicy};
use crate::remote::{ListRemoteBundleInfo, Remote, RemoteBundleInfo};
#[cfg(feature = "signature")]
use crate::signature::SignatureVerifier;
use crate::source::{BundleManifestMetadata, BundleSource};
use crate::{BundleDescriptor, BundleReader, Reader};
use dashmap::DashMap;
use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard};

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
  pub(crate) integrity_policy: IntegrityPolicy,
  #[cfg(feature = "integrity")]
  pub(crate) integrity_checker: IntegrityChecker,
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

  /// How a bundle's integrity metadata is treated when it is downloaded or installed
  /// (default: [`IntegrityPolicy::Optional`]). [`IntegrityPolicy::Off`] disables the check.
  #[cfg(feature = "integrity")]
  pub fn integrity_policy(mut self, policy: IntegrityPolicy) -> Self {
    self.integrity_policy = policy;
    self
  }

  /// Verifies that each bundle's integrity string was signed by the matching key.
  ///
  /// The signature signs the integrity string, not the bundle bytes, and is verified
  /// independently of the integrity check — keep [`UpdaterConfig::integrity_policy`]
  /// enabled for the signature to authenticate the bytes too.
  #[cfg(feature = "signature")]
  pub fn signature_verifier(mut self, verifier: SignatureVerifier) -> Self {
    self.signature_verifier = Some(verifier);
    self
  }

  /// Verifies `data` against the integrity and signature advertised for it. The two
  /// checks run independently.
  #[cfg(feature = "integrity")]
  async fn verify(
    &self,
    integrity: Option<&str>,
    signature: Option<&str>,
    data: &[u8],
  ) -> crate::Result<()> {
    crate::integrity::verify_integrity(
      self.integrity_policy,
      &self.integrity_checker,
      integrity,
      data,
    )
    .await?;
    #[cfg(feature = "signature")]
    if let Some(verifier) = &self.signature_verifier {
      crate::signature::verify_signature(verifier, integrity, signature).await?;
    }
    #[cfg(not(feature = "signature"))]
    let _ = signature;
    Ok(())
  }
}

pub struct Updater {
  source: Arc<BundleSource>,
  remote: Arc<Remote>,
  config: UpdaterConfig,
  // Per-bundle transaction lock.
  locks: DashMap<String, Arc<Mutex<()>>>,
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
      locks: DashMap::new(),
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
    // Serialize against concurrent installs/downloads of this same bundle.
    let _guard = self.lock_bundle(&bundle_name).await;
    // The bundle is parsed to validate that the response is a well-formed `.wvb`, but the
    // raw bytes are what gets written: they are what the integrity string covers, so
    // storing them verbatim keeps the on-disk file verifiable on every later load.
    let (info, _bundle, data) = match version {
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
      .config
      .verify(info.integrity.as_deref(), info.signature.as_deref(), &data)
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

    // Parse the staged file before activating it: a file that is not a well-formed bundle
    // must never become the active version, whether or not integrity is advertised for it.
    // The descriptor covers the header and index; reading it as a full `Bundle` would copy
    // the whole data section into a second buffer only to drop it.
    Reader::<BundleDescriptor>::read(&mut BundleReader::new(Cursor::new(&data)))?;

    #[cfg(feature = "integrity")]
    self
      .config
      .verify(
        metadata.integrity.as_deref(),
        metadata.signature.as_deref(),
        &data,
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
}
