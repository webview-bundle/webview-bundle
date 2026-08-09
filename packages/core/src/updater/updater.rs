use crate::remote::{BundleUpdate, Remote, RemoteGetUpdateOptions, Update};
use crate::source::{BundleManifestVersionData, BundleSource};
use crate::updater::tmp_file::TmpFile;
use crate::updater::update_file::UpdateFile;
use crate::updater::{
  DownloadOptions, DownloadResult, InstallBundleTarget, InstallResult, UpdaterOptions,
};
use crate::util::cancellation::Cancellation;
use crate::util::fs::rename_with_retry;
use crate::{BundleDescriptor, BundleReader, Reader};
use futures_util::{StreamExt, TryStreamExt};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Default, Clone)]
pub struct UpdaterBuilder {
  source: Option<Arc<BundleSource>>,
  remote: Option<Arc<Remote>>,
  update_filepath: Option<PathBuf>,
  options: UpdaterOptions,
}

impl UpdaterBuilder {
  #[must_use]
  pub fn source(mut self, source: Arc<BundleSource>) -> Self {
    self.source = Some(source);
    self
  }

  #[must_use]
  pub fn remote(mut self, remote: Arc<Remote>) -> Self {
    self.remote = Some(remote);
    self
  }

  #[must_use]
  pub fn update_filepath(mut self, update_filepath: &Path) -> Self {
    self.update_filepath = Some(update_filepath.to_path_buf());
    self
  }

  pub fn options(mut self, options: UpdaterOptions) -> Self {
    self.options = options;
    self
  }

  pub fn build(self) -> crate::Result<Updater> {
    if self.source.is_none() {
      return Err(crate::Error::invalid_updater_config(
        "\"source\" is required",
      ));
    }
    if self.remote.is_none() {
      return Err(crate::Error::invalid_updater_config(
        "\"remote\" is required",
      ));
    }
    if self.update_filepath.is_none() {
      return Err(crate::Error::invalid_updater_config(
        "\"update_filepath\" is required",
      ));
    }
    let updater = Updater {
      source: self.source.unwrap(),
      remote: self.remote.unwrap(),
      options: self.options,
      file: UpdateFile::new(&self.update_filepath.unwrap()),
      lock: Default::default(),
    };
    Ok(updater)
  }
}

static DOWNLOAD_SEQ: AtomicU64 = AtomicU64::new(0);

pub struct Updater {
  source: Arc<BundleSource>,
  remote: Arc<Remote>,
  options: UpdaterOptions,
  file: UpdateFile,
  // Jobs that result in actual changes to the file on disk acquire a lock to ensure they are
  // executed serially.
  // Note that, the reason individual locks are not maintained for each bundle is that installation
  // atomicity must be guaranteed for bundles with interdependencies (e.g., Micro-Frontends).
  lock: Arc<Mutex<()>>,
}

impl Updater {
  pub fn builder() -> UpdaterBuilder {
    UpdaterBuilder::default()
  }

  /// Fetches update information from the remote server and reduces it to the bundles that
  /// are not being served yet.
  ///
  /// The response is stored on disk so its etag can be replayed on the next call, and so an
  /// update that was fetched but not downloaded yet survives a restart: when the server
  /// answers `304 Not Modified`, the stored update is diffed instead.
  ///
  /// Returns `None` when the update carries nothing this source is missing.
  pub async fn get_update(&self) -> crate::Result<Option<Update>> {
    let prev = self.file.read().await?;

    let mut options = RemoteGetUpdateOptions::default();
    if let Some(prev) = &prev {
      if let Some(etag) = &prev.etag {
        options = options.etag(etag);
      }
    }
    if let Some(channel) = &self.options.channel {
      options = options.channel(channel);
    }
    // The signature is verified against the raw response body while it is still in the
    // client, so asking for it here is what makes the parsed update trustworthy.
    #[cfg(feature = "signature")]
    {
      if let Some(key_set) = self
        .options
        .signature
        .key_sets
        .as_ref()
        .and_then(|x| x.first())
      {
        options = options.expect_signature(key_set.clone());
      }
    }

    let resp = match self.remote.get_update(Some(options)).await? {
      Some(next) => {
        let changed = prev
          .as_ref()
          .is_none_or(|prev| prev.update.id != next.update.id);
        if changed {
          self.file.write(&next).await?;
        }
        next
      }
      // Not modified: whatever was stored is still the current update.
      None => match prev {
        Some(prev) => prev,
        None => return Ok(None),
      },
    };

    if resp.update.runtime_version > crate::UPDATE_RUNTIME_VERSION {
      return Ok(None);
    }

    let mut bundles = vec![];
    for bundle in resp.update.bundles.iter() {
      if self.is_update_needed(bundle).await? {
        bundles.push(bundle.clone());
      }
    }
    if bundles.is_empty() {
      return Ok(None);
    }

    Ok(Some(Update {
      bundles,
      ..resp.update
    }))
  }

  /// Whether `bundle` names a version other than the one the source resolves for it today,
  /// with a remote version taking precedence over the builtin one.
  async fn is_update_needed(&self, bundle: &BundleUpdate) -> crate::Result<bool> {
    let current = self.source.get_version(&bundle.name).await?;
    Ok(match current {
      Some(current) => current.version != bundle.version,
      None => true,
    })
  }

  /// Download a new version of a bundle and save into disk.
  ///
  /// Downloaded bundle does not activate automatically.
  /// To activate the downloaded version, use `Updater::install`.
  pub async fn download(
    &self,
    bundle_updates: &[BundleUpdate],
    options: Option<DownloadOptions>,
  ) -> crate::Result<Vec<DownloadResult>> {
    let options = options.unwrap_or_default();

    let _guard = self.lock(&options.timeout).await?;

    let cancellation = options.cancellation.clone().unwrap_or_default();
    let concurrency = options.concurrency.unwrap_or(3).max(1);
    let seq = DOWNLOAD_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let results = futures_util::stream::iter(bundle_updates.iter().map(|bundle_update| {
      let cancellation = cancellation.clone();
      async move {
        let result = self.download_one(seq, bundle_update, cancellation).await;
        Ok::<DownloadResult, crate::Error>(DownloadResult {
          update: bundle_update.clone(),
          result,
        })
      }
    }))
    .buffer_unordered(concurrency)
    .try_collect::<Vec<_>>()
    .await?;

    let staged = results
      .iter()
      .filter(|x| x.result.is_ok())
      .map(|x| {
        (
          x.update.name.to_owned(),
          x.update.version.to_owned(),
          BundleManifestVersionData {
            integrity: x.update.integrity.to_owned(),
            metadata: x.update.metadata.to_owned(),
          },
        )
      })
      .collect::<Vec<_>>();
    self.source.stage_remote_bundles(&staged).await?;

    Ok(results)
  }

  async fn download_one(
    &self,
    seq: u64,
    bundle_update: &BundleUpdate,
    cancellation: Cancellation,
  ) -> crate::Result<()> {
    let filepath = self
      .source
      .get_remote_bundle_filepath(&bundle_update.name, &bundle_update.version)?;
    let tmp_file = TmpFile::new(&filepath, seq);

    let download_url = bundle_update.download_url.clone().unwrap_or(
      self
        .remote
        .default_download_url(&bundle_update.name, &bundle_update.version),
    );

    self
      .remote
      .download(
        download_url,
        Path::new(&tmp_file.filepath()),
        Some(cancellation),
      )
      .await?;
    rename_with_retry(&tmp_file.filepath(), &filepath).await?;

    Ok(())
  }

  /// Activates downloaded versions so the protocol begins serving them.
  pub async fn install(
    &self,
    targets: &[InstallBundleTarget],
  ) -> crate::Result<Vec<InstallResult>> {
    let _guard = self.lock(&None).await?;

    let groups = group_atomic_targets(targets);
    let mut errors = targets.iter().map(|_| None).collect::<Vec<_>>();
    let mut failed_groups: HashMap<usize, (String, String)> = HashMap::new();

    for (index, target) in targets.iter().enumerate() {
      if failed_groups.contains_key(&groups[index]) {
        continue;
      }
      if let Err(e) = self.verify_install_target(target).await {
        failed_groups.insert(groups[index], (target.name.clone(), target.version.clone()));
        errors[index] = Some(e);
      }
    }

    let installed = targets
      .iter()
      .enumerate()
      .filter(|(index, _)| !failed_groups.contains_key(&groups[*index]))
      .map(|(_, target)| (target.name.clone(), target.version.clone()))
      .collect::<Vec<_>>();

    self.source.update_remote_versions(&installed).await?;

    let bundle_names = installed
      .into_iter()
      .map(|(bundle_name, _)| {
        self.source.unload(&bundle_name);
        bundle_name
      })
      .collect::<Vec<_>>();
    let _ = self.source.prune_remote_bundles(&bundle_names).await;

    let results = targets
      .iter()
      .zip(errors)
      .enumerate()
      .map(|(index, (target, error))| {
        let result = match error {
          Some(error) => Err(error),
          None => match failed_groups.get(&groups[index]) {
            Some((bundle_name, version)) => {
              Err(crate::Error::install_atomic_failed(bundle_name, version))
            }
            None => Ok(()),
          },
        };
        InstallResult {
          target: target.clone(),
          result,
        }
      })
      .collect();

    Ok(results)
  }

  async fn verify_install_target(&self, target: &InstallBundleTarget) -> crate::Result<()> {
    let staged = self
      .source
      .get_remote_staged_version(&target.name)
      .await?
      .filter(|x| x == &target.version);
    if staged.is_none() {
      return Err(crate::Error::bundle_entry_not_exists(
        &target.name,
        &target.version,
      ));
    }

    let filepath = self
      .source
      .get_remote_bundle_filepath(&target.name, &target.version)?;
    let data = tokio::fs::read(&filepath).await?;

    // Read bundle data to expect the file is webview bundle formatted.
    Reader::<BundleDescriptor>::read(&mut BundleReader::new(Cursor::new(&data)))?;

    #[cfg(feature = "integrity")]
    {
      let version_data = self
        .source
        .get_remote_version_data(&target.name, &target.version)
        .await?
        .ok_or_else(|| crate::Error::bundle_entry_not_exists(&target.name, &target.version))?;
      crate::integrity::verify_integrity(
        &self.options.integrity.policy,
        version_data.integrity.as_deref(),
        &data,
      )?;
    }

    Ok(())
  }

  async fn lock(&self, timeout: &Option<u64>) -> crate::Result<OwnedMutexGuard<()>> {
    let timeout = timeout.unwrap_or(0);
    if timeout == 0 {
      return Ok(self.lock.clone().lock_owned().await);
    }
    tokio::time::timeout(
      tokio::time::Duration::from_millis(timeout),
      self.lock.clone().lock_owned(),
    )
    .await
    .map_err(|_| crate::Error::Timeout)
  }
}

/// Assigns each target the id of the atomic group it belongs to.
///
/// A target is grouped with every target it names in `atomic`, and grouping is transitive:
/// `a -> [b]` and `b -> [c]` puts all three in one group. Names that match no target act as a
/// shared label, so `a -> [ui]` and `b -> [ui]` are grouped as well.
fn group_atomic_targets(targets: &[InstallBundleTarget]) -> Vec<usize> {
  let mut parents = (0..targets.len()).collect::<Vec<_>>();
  let mut groups_by_name: HashMap<&str, usize> = HashMap::new();

  for (index, target) in targets.iter().enumerate() {
    if let Some(prev) = groups_by_name.insert(target.name.as_str(), index) {
      union_group(&mut parents, prev, index);
    }
  }

  for (index, target) in targets.iter().enumerate() {
    let Some(atomic) = &target.atomic else {
      continue;
    };
    for name in atomic {
      match groups_by_name.get(name.as_str()).copied() {
        Some(other) => union_group(&mut parents, other, index),
        None => {
          groups_by_name.insert(name.as_str(), index);
        }
      }
    }
  }

  (0..targets.len())
    .map(|index| find_group(&mut parents, index))
    .collect()
}

fn find_group(parents: &mut [usize], index: usize) -> usize {
  let mut index = index;
  while parents[index] != index {
    parents[index] = parents[parents[index]];
    index = parents[index];
  }
  index
}

fn union_group(parents: &mut [usize], a: usize, b: usize) {
  let a = find_group(parents, a);
  let b = find_group(parents, b);
  if a != b {
    parents[b] = a;
  }
}

// #[cfg(all(test, feature = "testing"))]
// mod tests {
//   use super::*;
//   use crate::ErrorCode;
//   use crate::testing::{MockBundle, MockSource, TempDir};
//   use httpmock::MockServer;
//
//   fn system(base_url: impl Into<String>) -> (MockSource, TempDir, Arc<BundleSource>, Updater) {
//     let mock_source = MockSource::new();
//     let temp = TempDir::new();
//     let source = Arc::new(mock_source.get_source());
//     let remote = Arc::new(Remote::builder().base_url(base_url).build().unwrap());
//     let updater = Updater::builder()
//       .source(source.clone())
//       .remote(remote)
//       .update_filepath(&temp.dir().join("update.json"))
//       .build()
//       .unwrap();
//     (mock_source, temp, source, updater)
//   }
//
//   fn serve(server: &MockServer, bundle: &MockBundle) {
//     let path = format!("/bundles/{}/{}", bundle.name(), bundle.version());
//     let data = bundle.bundle_data();
//     server.mock(|when, then| {
//       when.method("GET").path(path);
//       then.status(200).body(data);
//     });
//   }
//
//   fn bundle_update(bundle: &MockBundle) -> BundleUpdate {
//     BundleUpdate {
//       name: bundle.name().to_owned(),
//       version: bundle.version().to_owned(),
//       download_url: None,
//       integrity: None,
//     }
//   }
//
//   fn install_target(bundle: &MockBundle, atomic: Option<&[&str]>) -> InstallBundleTarget {
//     InstallBundleTarget {
//       name: bundle.name().to_owned(),
//       version: bundle.version().to_owned(),
//       atomic: atomic.map(|x| x.iter().map(|y| y.to_string()).collect()),
//     }
//   }
//
//   fn downloaded<'a>(results: &'a [DownloadResult], name: &str) -> &'a DownloadResult {
//     results.iter().find(|x| x.update.name == name).unwrap()
//   }
//
//   fn installed<'a>(results: &'a [InstallResult], name: &str) -> &'a InstallResult {
//     results.iter().find(|x| x.target.name == name).unwrap()
//   }
//
//   async fn stage(source: &BundleSource, bundle: &MockBundle) {
//     source
//       .write_remote_bundle_data(
//         bundle.name(),
//         bundle.version(),
//         &bundle.bundle_data(),
//         bundle.metadata(),
//       )
//       .await
//       .unwrap();
//     source
//       .stage_remote_version(bundle.name(), bundle.version())
//       .await
//       .unwrap();
//   }
//
//   async fn current_version(source: &BundleSource, bundle_name: &str) -> Option<String> {
//     source
//       .get_version(bundle_name)
//       .await
//       .unwrap()
//       .map(|x| x.version)
//   }
//
//   #[tokio::test]
//   async fn downloads_bundles() {
//     let server = MockServer::start();
//     let a = MockBundle::new("a", "1.0.0");
//     let b = MockBundle::new("b", "2.0.0");
//     serve(&server, &a);
//     serve(&server, &b);
//     let (_mock_source, _temp, source, updater) = system(server.base_url());
//
//     let results = updater
//       .download(&[bundle_update(&a), bundle_update(&b)], None)
//       .await
//       .unwrap();
//
//     assert_eq!(results.len(), 2);
//     assert!(results.iter().all(|x| x.result.is_ok()));
//     assert_eq!(
//       tokio::fs::read(source.get_remote_bundle_filepath("a", "1.0.0").unwrap())
//         .await
//         .unwrap(),
//       a.bundle_data()
//     );
//     assert_eq!(
//       tokio::fs::read(source.get_remote_bundle_filepath("b", "2.0.0").unwrap())
//         .await
//         .unwrap(),
//       b.bundle_data()
//     );
//   }
//
//   #[tokio::test]
//   async fn downloads_from_custom_url() {
//     let server = MockServer::start();
//     let a = MockBundle::new("a", "1.0.0");
//     let data = a.bundle_data();
//     server.mock(|when, then| {
//       when.method("GET").path("/custom/a.wvb");
//       then.status(200).body(data);
//     });
//     let (_mock_source, _temp, source, updater) = system(server.base_url());
//     let mut update = bundle_update(&a);
//     update.download_url = Some(server.url("/custom/a.wvb"));
//
//     let results = updater.download(&[update], None).await.unwrap();
//
//     assert!(results[0].result.is_ok());
//     assert_eq!(
//       tokio::fs::read(source.get_remote_bundle_filepath("a", "1.0.0").unwrap())
//         .await
//         .unwrap(),
//       a.bundle_data()
//     );
//   }
//
//   #[tokio::test]
//   async fn download_reports_failure_per_bundle() {
//     let server = MockServer::start();
//     let a = MockBundle::new("a", "1.0.0");
//     let b = MockBundle::new("b", "2.0.0");
//     serve(&server, &a);
//     let (_mock_source, _temp, source, updater) = system(server.base_url());
//
//     let results = updater
//       .download(&[bundle_update(&a), bundle_update(&b)], None)
//       .await
//       .unwrap();
//
//     assert!(downloaded(&results, "a").result.is_ok());
//     assert!(downloaded(&results, "b").result.is_err());
//     assert!(
//       !source
//         .get_remote_bundle_filepath("b", "2.0.0")
//         .unwrap()
//         .exists()
//     );
//   }
//
//   #[tokio::test]
//   async fn download_cancelled_already() {
//     let server = MockServer::start();
//     let a = MockBundle::new("a", "1.0.0");
//     serve(&server, &a);
//     let (_mock_source, _temp, source, updater) = system(server.base_url());
//     let cancellation = Cancellation::new();
//     cancellation.cancel();
//     let options = DownloadOptions {
//       cancellation: Some(cancellation),
//       ..Default::default()
//     };
//
//     let results = updater
//       .download(&[bundle_update(&a)], Some(options))
//       .await
//       .unwrap();
//
//     assert_eq!(
//       results[0].result.as_ref().unwrap_err().code(),
//       ErrorCode::Cancelled
//     );
//     assert!(
//       !source
//         .get_remote_bundle_filepath("a", "1.0.0")
//         .unwrap()
//         .exists()
//     );
//   }
//
//   #[tokio::test]
//   async fn installs_staged_bundles() {
//     let (_mock_source, _temp, source, updater) = system("http://127.0.0.1:1");
//     let a = MockBundle::new("a", "1.0.0");
//     let b = MockBundle::new("b", "2.0.0");
//     stage(&source, &a).await;
//     stage(&source, &b).await;
//
//     let results = updater
//       .install(&[install_target(&a, None), install_target(&b, None)])
//       .await
//       .unwrap();
//
//     assert!(results.iter().all(|x| x.result.is_ok()));
//     assert_eq!(
//       current_version(&source, "a").await.as_deref(),
//       Some("1.0.0")
//     );
//     assert_eq!(
//       current_version(&source, "b").await.as_deref(),
//       Some("2.0.0")
//     );
//     assert_eq!(source.get_remote_staged_version("a").await.unwrap(), None);
//   }
//
//   #[tokio::test]
//   async fn install_rejects_version_which_is_not_staged() {
//     let (_mock_source, _temp, source, updater) = system("http://127.0.0.1:1");
//     let a = MockBundle::new("a", "1.0.0");
//     let other = MockBundle::new("a", "9.9.9");
//     stage(&source, &a).await;
//
//     let results = updater
//       .install(&[install_target(&other, None)])
//       .await
//       .unwrap();
//
//     assert!(results[0].result.is_err());
//     assert_eq!(current_version(&source, "a").await, None);
//   }
//
//   #[tokio::test]
//   async fn install_fails_atomic_group_together() {
//     let (_mock_source, _temp, source, updater) = system("http://127.0.0.1:1");
//     let a = MockBundle::new("a", "1.0.0");
//     let b = MockBundle::new("b", "2.0.0");
//     stage(&source, &a).await;
//
//     let results = updater
//       .install(&[install_target(&a, Some(&["b"])), install_target(&b, None)])
//       .await
//       .unwrap();
//
//     assert!(matches!(
//       &installed(&results, "a").result,
//       Err(crate::Error::InstallAtomicFailed { bundle_name, version })
//         if bundle_name == "b" && version == "2.0.0"
//     ));
//     assert!(installed(&results, "b").result.is_err());
//     assert_eq!(current_version(&source, "a").await, None);
//     assert_eq!(
//       source
//         .get_remote_staged_version("a")
//         .await
//         .unwrap()
//         .as_deref(),
//       Some("1.0.0")
//     );
//   }
//
//   #[tokio::test]
//   async fn installs_other_targets_when_group_fails() {
//     let (_mock_source, _temp, source, updater) = system("http://127.0.0.1:1");
//     let a = MockBundle::new("a", "1.0.0");
//     let b = MockBundle::new("b", "2.0.0");
//     let c = MockBundle::new("c", "3.0.0");
//     stage(&source, &a).await;
//     stage(&source, &c).await;
//
//     let results = updater
//       .install(&[
//         install_target(&a, Some(&["b"])),
//         install_target(&b, None),
//         install_target(&c, None),
//       ])
//       .await
//       .unwrap();
//
//     assert!(installed(&results, "c").result.is_ok());
//     assert!(installed(&results, "a").result.is_err());
//     assert!(installed(&results, "b").result.is_err());
//     assert_eq!(
//       current_version(&source, "c").await.as_deref(),
//       Some("3.0.0")
//     );
//     assert_eq!(current_version(&source, "a").await, None);
//   }
// }
