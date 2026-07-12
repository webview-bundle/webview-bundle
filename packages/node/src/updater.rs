use crate::integrity::IntegrityPolicy;
use crate::js::{JsCallback, JsCallbackExt};
use crate::remote::{ListRemoteBundleInfo, Remote, RemoteBundleInfo};
use crate::signature::SignatureVerifier;
use crate::source::BundleSource;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::sync::Arc;
use wvb::integrity::IntegrityChecker;
use wvb::updater;

/// Information about a bundle update.
///
/// @property {string} name - Bundle name
/// @property {string} version - Remote version available
/// @property {string} [localVersion] - Currently installed version
/// @property {boolean} isAvailable - Whether an update is available
/// @property {string} [etag] - ETag for caching
/// @property {string} [integrity] - Integrity hash (e.g., "sha384-...")
/// @property {string} [signature] - Digital signature
/// @property {string} [lastModified] - Last modified timestamp
///
/// @example
/// ```typescript
/// const updateInfo = await updater.getUpdate('app');
/// if (updateInfo.isAvailable) {
///   console.log(`Update available: ${updateInfo.localVersion} → ${updateInfo.version}`);
///   await updater.download('app');
/// }
/// ```
#[napi(object)]
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

impl From<BundleUpdateInfo> for updater::BundleUpdateInfo {
  fn from(value: BundleUpdateInfo) -> Self {
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

pub(crate) type UpdateIntegrityChecker = JsCallback<FnArgs<(Buffer, String)>, Promise<bool>>;

/// Configuration options for the updater.
///
/// @property {string} [channel] - Update channel (e.g., "stable", "beta")
/// @property {IntegrityPolicy} [integrityPolicy] - Policy for integrity verification
/// @property {Function} [integrityChecker] - Custom integrity verification function
/// @property {SignatureVerifierOptions | Function} [signatureVerifier] - Signature verification config or custom function. A custom function receives `message` — the UTF-8 bytes of the bundle's integrity string (e.g. `sha256:<base64>`), which is what the signature covers — and NOT the bundle bytes.
///
/// @example
/// ```typescript
/// const updater = new Updater(source, remote, {
///   channel: 'stable',
///   integrityPolicy: 'strict',
///   signatureVerifier: {
///     algorithm: 'ed25519',
///     key: {
///       format: 'spkiPem',
///       data: publicKeyPem,
///     },
///   },
/// });
/// ```
///
/// @example
/// ```typescript
/// // Custom verification functions
/// const updater = new Updater(source, remote, {
///   integrityChecker: async (data, integrity) => {
///     // Custom integrity verification
///     return true;
///   },
///   signatureVerifier: async (message, signature) => {
///     // Custom signature verification
///     return true;
///   },
/// });
/// ```
#[napi(object, object_to_js = false)]
pub struct UpdaterOptions {
  pub channel: Option<String>,
  pub integrity_policy: Option<IntegrityPolicy>,
  #[napi(ts_type = "(data: Uint8Array, integrity: string) => Promise<boolean>")]
  pub integrity_checker: Option<UpdateIntegrityChecker>,
  #[napi(
    ts_type = "SignatureVerifierOptions | ((message: Uint8Array, signature: string) => Promise<boolean>)"
  )]
  pub signature_verifier: Option<SignatureVerifier>,
}

impl From<UpdaterOptions> for updater::UpdaterConfig {
  fn from(value: UpdaterOptions) -> Self {
    let mut config = updater::UpdaterConfig::default();
    if let Some(channel) = value.channel {
      config = config.channel(channel);
    }
    if let Some(policy) = value.integrity_policy {
      config = config.integrity_policy(policy.into());
    }
    if let Some(checker) = value.integrity_checker {
      config = config.integrity_checker(IntegrityChecker::Custom(Arc::new(
        move |data, signature| {
          let buffer = Buffer::from(data);
          let signature = signature.to_string();
          let callback = Arc::clone(&checker);
          Box::pin(async move {
            let ret = callback
              .invoke_async((buffer, signature).into())
              .await?
              .await?;
            Ok(ret)
          })
        },
      )));
    }
    if let Some(verifier) = value.signature_verifier {
      config = config.signature_verifier(verifier.inner);
    }
    config
  }
}

/// Bundle updater for managing updates from a remote server.
///
/// The updater coordinates between a local bundle source and remote server,
/// handling update checks, downloads, integrity verification, and signature validation.
///
/// @example
/// ```typescript
/// import { Updater, BundleSource, Remote } from '@wvb/node';
///
/// const source = new BundleSource({
///   builtinDir: './bundles/builtin',
///   remoteDir: './bundles/remote',
/// });
///
/// const remote = new Remote('https://updates.example.com');
///
/// const updater = new Updater(source, remote, {
///   channel: 'stable',
///   integrityPolicy: 'strict',
///   signatureVerifier: {
///     algorithm: 'ed25519',
///     key: {
///       format: 'spkiPem',
///       data: publicKeyPem,
///     },
///   },
/// });
///
/// // Check for updates
/// const updateInfo = await updater.getUpdate('app');
/// if (updateInfo.isAvailable) {
///   console.log(`Update available: ${updateInfo.version}`);
///   await updater.download('app');
///   await updater.install('app', updateInfo.version);
/// }
/// ```
#[napi]
pub struct Updater {
  pub(crate) inner: updater::Updater,
}

#[napi]
impl Updater {
  /// Creates a new updater instance.
  ///
  /// @param {BundleSource} source - Bundle source for storing downloaded bundles
  /// @param {Remote} remote - Remote client for fetching bundles
  /// @param {UpdaterOptions} [options] - Optional updater configuration
  ///
  /// @example
  /// ```typescript
  /// const updater = new Updater(source, remote, {
  ///   channel: 'stable',
  ///   integrityPolicy: 'strict',
  /// });
  /// ```
  #[napi(constructor)]
  pub fn new(
    source: &BundleSource,
    remote: &Remote,
    options: Option<UpdaterOptions>,
  ) -> crate::Result<Updater> {
    let source = source.inner.clone();
    let remote = remote.inner.clone();
    Ok(Updater {
      inner: updater::Updater::new(source, remote, options.map(Into::into)),
    })
  }

  /// Lists all available bundles on the remote server.
  ///
  /// @returns {Promise<ListRemoteBundleInfo[]>} Array of remote bundle information
  ///
  /// @example
  /// ```typescript
  /// const remotes = await updater.listRemotes();
  /// for (const bundle of remotes) {
  ///   console.log(`${bundle.name}: ${bundle.version}`);
  /// }
  /// ```
  #[napi]
  pub async fn list_remotes(&self) -> crate::Result<Vec<ListRemoteBundleInfo>> {
    let remotes = self
      .inner
      .list_remotes()
      .await?
      .into_iter()
      .map(ListRemoteBundleInfo::from)
      .collect::<Vec<_>>();
    Ok(remotes)
  }

  /// Checks if an update is available for a specific bundle.
  ///
  /// Compares the local version with the remote version to determine if an update exists.
  ///
  /// @param {string} bundleName - Name of the bundle to check
  /// @returns {Promise<BundleUpdateInfo>} Update information
  ///
  /// @example
  /// ```typescript
  /// const updateInfo = await updater.getUpdate('app');
  /// if (updateInfo.isAvailable) {
  ///   console.log(`Update available: ${updateInfo.localVersion} → ${updateInfo.version}`);
  /// } else {
  ///   console.log('Already up to date');
  /// }
  /// ```
  #[napi]
  pub async fn get_update(&self, bundle_name: String) -> crate::Result<BundleUpdateInfo> {
    let update = self.inner.get_update(&bundle_name).await?;
    Ok(BundleUpdateInfo::from(update))
  }

  /// Downloads a bundle update from remote server.
  ///
  /// Downloads the specified bundle version (or the latest if not specified),
  /// verifies integrity and signature if configured, and download it to the remote directory.
  ///
  /// @param {string} bundleName - Name of the bundle to download
  /// @param {string} [version] - Specific version to download (defaults to latest)
  /// @returns {Promise<RemoteBundleInfo>} Information about the downloaded bundle
  ///
  /// @example
  /// ```typescript
  /// // Download latest version
  /// const info = await updater.download('app');
  /// console.log(`Downloaded ${info.name} v${info.version}`);
  /// ```
  ///
  /// @example
  /// ```typescript
  /// // Download specific version
  /// const info = await updater.download('app', '1.2.3');
  /// console.log(`Downloaded ${info.name} v${info.version}`);
  /// ```
  #[napi]
  pub async fn download(
    &self,
    bundle_name: String,
    version: Option<String>,
  ) -> crate::Result<RemoteBundleInfo> {
    let info = self.inner.download(bundle_name, version).await?;
    Ok(info.into())
  }

  /// Activates a previously downloaded bundle version.
  ///
  /// The version must already be staged in the remote source (via
  /// {@link Updater.download}). When integrity/signature verification is
  /// configured, the staged bundle is verified before activation. On success the
  /// current version is updated so the protocol begins serving it, the cached
  /// descriptor is dropped, and stale staged versions are pruned.
  ///
  /// Concurrent `install`/`download` calls for the same bundle are serialized,
  /// so this never races a download or another install of the same bundle.
  ///
  /// @param {string} bundleName - Name of the bundle to activate
  /// @param {string} version - The downloaded version to activate
  ///
  /// @example
  /// ```typescript
  /// await updater.download('app', '1.2.0');
  /// // ...later, active the latest version:
  /// await updater.install('app', '1.2.0');
  /// ```
  #[napi]
  pub async fn install(&self, bundle_name: String, version: String) -> crate::Result<()> {
    self.inner.install(bundle_name, version).await?;
    Ok(())
  }
}
