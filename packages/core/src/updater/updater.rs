use crate::remote::{BundleUpdate, Remote, RemoteGetUpdateOptions, Update};
use crate::source::{BundleManifestVersionData, BundleSource};
use crate::updater::tmp_file::TmpFile;
use crate::updater::update_file::UpdateFile;
use crate::updater::{
  UpdaterDownloadOptions, UpdaterDownloadResult, UpdaterGetUpdateOptions,
  UpdaterInstallBundleTarget, UpdaterInstallResult, UpdaterOptions,
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
  pub async fn get_update(
    &self,
    options: Option<UpdaterGetUpdateOptions>,
  ) -> crate::Result<Option<Update>> {
    let prev = self.file.read().await?;

    let mut opts = RemoteGetUpdateOptions::default();
    if let Some(prev) = &prev {
      if let Some(etag) = &prev.etag {
        opts = opts.etag(etag);
      }
    }
    if let Some(channel) = &self.options.channel {
      opts = opts.channel(channel);
    }

    #[cfg(feature = "signature")]
    {
      if let Some(key_id) = options.and_then(|x| x.expect_signature_key_id).as_ref() {
        match self
          .options
          .signature
          .key_sets
          .as_ref()
          .and_then(|x| x.iter().find(|key_set| &key_set.id == key_id))
        {
          Some(key) => {
            opts = opts.expect_signature(key.clone());
          }
          None => {
            return Err(crate::Error::expect_signature_not_found(key_id));
          }
        }
      }
    }

    let resp = match self.remote.get_update(Some(opts)).await? {
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
    options: Option<UpdaterDownloadOptions>,
  ) -> crate::Result<Vec<UpdaterDownloadResult>> {
    let options = options.unwrap_or_default();

    let _guard = self.lock(&options.timeout).await?;

    let cancellation = options.cancellation.clone().unwrap_or_default();
    let concurrency = options.concurrency.unwrap_or(3).max(1);
    let seq = DOWNLOAD_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    // The jobs own their `BundleUpdate` rather than borrowing from `bundle_updates`: a
    // closure returning a future that borrows its argument is not general enough over
    // lifetimes to survive `tokio::spawn`, which is exactly where downloads are driven from.
    let jobs = bundle_updates.to_vec();
    let results = futures_util::stream::iter(jobs.into_iter().map(|bundle_update| {
      let cancellation = cancellation.clone();
      async move {
        let result = self.download_one(seq, &bundle_update, cancellation).await;
        Ok::<UpdaterDownloadResult, crate::Error>(UpdaterDownloadResult {
          update: bundle_update,
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
    rename_with_retry(tmp_file.filepath(), &filepath).await?;

    Ok(())
  }

  /// Activates downloaded versions so the protocol begins serving them.
  pub async fn install(
    &self,
    targets: &[UpdaterInstallBundleTarget],
  ) -> crate::Result<Vec<UpdaterInstallResult>> {
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
        UpdaterInstallResult {
          target: target.clone(),
          result,
        }
      })
      .collect();

    Ok(results)
  }

  async fn verify_install_target(&self, target: &UpdaterInstallBundleTarget) -> crate::Result<()> {
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
fn group_atomic_targets(targets: &[UpdaterInstallBundleTarget]) -> Vec<usize> {
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

#[cfg(all(test, feature = "testing"))]
mod tests {
  use super::*;
  use crate::ErrorCode;
  use crate::remote::RemoteUpdateResponse;
  use crate::testing::{TempDir, TestingBundle, TestingRemoteServer, TestingSourceBuilder};

  const OFFLINE_URL: &str = "http://127.0.0.1:1";

  fn update_filepath() -> PathBuf {
    TempDir::new().dir().join("update.json")
  }

  fn build_updater(
    source: &Arc<BundleSource>,
    base_url: &str,
    update_filepath: &Path,
    options: UpdaterOptions,
  ) -> Updater {
    Updater::builder()
      .source(source.clone())
      .remote(Arc::new(
        Remote::builder().base_url(base_url).build().unwrap(),
      ))
      .update_filepath(update_filepath)
      .options(options)
      .build()
      .unwrap()
  }

  fn updater(source: &Arc<BundleSource>, base_url: &str) -> Updater {
    build_updater(
      source,
      base_url,
      &update_filepath(),
      UpdaterOptions::default(),
    )
  }

  fn remote_server(bundles: &[(&str, &str)]) -> TestingRemoteServer {
    let mut server = TestingRemoteServer::new();
    for (name, version) in bundles {
      server.insert_bundle(TestingBundle::new(*name, *version));
    }
    server
  }

  fn builtin_source(bundles: &[(&str, &str)]) -> Arc<BundleSource> {
    let mut builder = TestingSourceBuilder::new();
    for (name, version) in bundles {
      builder.add_builtin_bundle(TestingBundle::new(*name, *version));
    }
    Arc::new(builder.build().unwrap())
  }

  fn empty_source() -> Arc<BundleSource> {
    Arc::new(TestingSourceBuilder::new().build().unwrap())
  }

  fn staged_source(bundles: &[(&str, &str)]) -> Arc<BundleSource> {
    let mut builder = TestingSourceBuilder::new();
    for (name, version) in bundles {
      builder.add_remote_bundle(TestingBundle::new(*name, *version));
      builder.set_remote_staged_version(*name, *version);
    }
    Arc::new(builder.build().unwrap())
  }

  fn bundle_update(name: &str, version: &str) -> BundleUpdate {
    BundleUpdate {
      name: name.to_owned(),
      version: version.to_owned(),
      download_url: None,
      integrity: None,
      metadata: None,
    }
  }

  fn install_target(
    name: &str,
    version: &str,
    atomic: Option<&[&str]>,
  ) -> UpdaterInstallBundleTarget {
    UpdaterInstallBundleTarget {
      name: name.to_owned(),
      version: version.to_owned(),
      atomic: atomic.map(|x| x.iter().map(|y| (*y).to_owned()).collect()),
    }
  }

  fn updated_bundles(update: &Update) -> Vec<(&str, &str)> {
    update
      .bundles
      .iter()
      .map(|x| (x.name.as_str(), x.version.as_str()))
      .collect()
  }

  async fn current_version(source: &BundleSource, bundle_name: &str) -> Option<String> {
    source
      .get_version(bundle_name)
      .await
      .unwrap()
      .map(|x| x.version)
  }

  #[track_caller]
  fn download_result<'a>(
    results: &'a [UpdaterDownloadResult],
    bundle_name: &str,
  ) -> &'a crate::Result<()> {
    &results
      .iter()
      .find(|x| x.update.name == bundle_name)
      .unwrap()
      .result
  }

  #[track_caller]
  fn install_result<'a>(
    results: &'a [UpdaterInstallResult],
    bundle_name: &str,
  ) -> &'a crate::Result<()> {
    &results
      .iter()
      .find(|x| x.target.name == bundle_name)
      .unwrap()
      .result
  }

  #[track_caller]
  fn error_code(result: &crate::Result<()>) -> ErrorCode {
    result.as_ref().unwrap_err().code()
  }

  #[tokio::test]
  async fn get_update_returns_bundles_which_are_not_served_yet() {
    let mut server = remote_server(&[("app", "1.1.0"), ("admin", "0.1.0")]);
    server.set_current_version("app", "1.1.0");
    server.set_current_version("admin", "0.1.0");
    let source = builtin_source(&[("app", "1.0.0"), ("admin", "0.1.0")]);
    let updater = updater(&source, &server.base_url());

    let update = updater.get_update(None).await.unwrap().unwrap();

    assert_eq!(updated_bundles(&update), vec![("app", "1.1.0")]);
  }

  #[tokio::test]
  async fn get_update_returns_none_when_every_bundle_is_served() {
    let mut server = remote_server(&[("app", "1.0.0")]);
    server.set_current_version("app", "1.0.0");
    let source = builtin_source(&[("app", "1.0.0")]);
    let updater = updater(&source, &server.base_url());

    assert_eq!(updater.get_update(None).await.unwrap(), None);
  }

  #[tokio::test]
  async fn get_update_stores_the_update_it_fetched() {
    let mut server = remote_server(&[("app", "1.1.0")]);
    server.set_current_version("app", "1.1.0");
    let source = builtin_source(&[("app", "1.0.0")]);
    let filepath = update_filepath();
    let updater = build_updater(
      &source,
      &server.base_url(),
      &filepath,
      UpdaterOptions::default(),
    );

    let update = updater.get_update(None).await.unwrap().unwrap();

    let stored = tokio::fs::read(&filepath).await.unwrap();
    let stored = serde_json::from_slice::<RemoteUpdateResponse>(&stored).unwrap();
    assert_eq!(stored.update.id, update.id);
    assert!(stored.etag.is_some());
  }

  #[tokio::test]
  async fn get_update_returns_the_stored_update_when_it_is_not_modified() {
    let mut server = remote_server(&[("app", "1.1.0")]);
    server.set_current_version("app", "1.1.0");
    let source = builtin_source(&[("app", "1.0.0")]);
    let filepath = update_filepath();
    build_updater(
      &source,
      &server.base_url(),
      &filepath,
      UpdaterOptions::default(),
    )
    .get_update(None)
    .await
    .unwrap()
    .unwrap();

    let raw = tokio::fs::read(&filepath).await.unwrap();
    let mut stored = serde_json::from_slice::<RemoteUpdateResponse>(&raw).unwrap();
    stored.update.bundles = vec![bundle_update("app", "1.2.0")];
    tokio::fs::write(&filepath, serde_json::to_vec(&stored).unwrap())
      .await
      .unwrap();

    let update = build_updater(
      &source,
      &server.base_url(),
      &filepath,
      UpdaterOptions::default(),
    )
    .get_update(None)
    .await
    .unwrap()
    .unwrap();

    assert_eq!(updated_bundles(&update), vec![("app", "1.2.0")]);
  }

  #[tokio::test]
  async fn get_update_fetches_the_configured_channel() {
    let mut server = remote_server(&[("app", "1.0.0"), ("app", "2.0.0")]);
    server.set_current_version("app", "1.0.0");
    server.set_channel_current_version("beta", "app", "2.0.0");
    let source = builtin_source(&[("app", "1.0.0")]);
    let updater = build_updater(
      &source,
      &server.base_url(),
      &update_filepath(),
      UpdaterOptions::default().channel("beta"),
    );

    let update = updater.get_update(None).await.unwrap().unwrap();

    assert_eq!(updated_bundles(&update), vec![("app", "2.0.0")]);
  }

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  #[tokio::test]
  async fn get_update_verifies_the_signature_of_the_expected_key() {
    use crate::updater::UpdaterSignatureOptions;

    let mut server = remote_server(&[("app", "1.1.0")]);
    server.set_current_version("app", "1.1.0");
    server.insert_signature_key("default", [7u8; 32]);
    let source = builtin_source(&[("app", "1.0.0")]);
    let options = UpdaterOptions::default().signature(
      UpdaterSignatureOptions::default().key_set(server.signature_key_set("default").unwrap()),
    );
    let updater = build_updater(&source, &server.base_url(), &update_filepath(), options);
    let get_update_options = UpdaterGetUpdateOptions::default().expect_signature_key_id("default");

    let update = updater
      .get_update(Some(get_update_options))
      .await
      .unwrap()
      .unwrap();

    assert_eq!(updated_bundles(&update), vec![("app", "1.1.0")]);
  }

  #[cfg(feature = "signature")]
  #[tokio::test]
  async fn get_update_errors_when_the_expected_key_is_not_configured() {
    let mut server = remote_server(&[("app", "1.1.0")]);
    server.set_current_version("app", "1.1.0");
    let source = builtin_source(&[("app", "1.0.0")]);
    let updater = updater(&source, &server.base_url());
    let options = UpdaterGetUpdateOptions::default().expect_signature_key_id("default");

    let err = updater.get_update(Some(options)).await.unwrap_err();

    assert_eq!(err.code(), ErrorCode::ExpectSignatureNotFound);
  }

  #[tokio::test]
  async fn download_writes_the_bundle_into_the_source() {
    let server = remote_server(&[("app", "1.0.0")]);
    let source = empty_source();
    let updater = updater(&source, &server.base_url());

    updater
      .download(&[bundle_update("app", "1.0.0")], None)
      .await
      .unwrap();

    let filepath = source.get_remote_bundle_filepath("app", "1.0.0").unwrap();
    assert_eq!(
      tokio::fs::read(&filepath).await.unwrap(),
      TestingBundle::new("app", "1.0.0")
        .make_bundle_data()
        .unwrap()
    );
  }

  #[tokio::test]
  async fn download_stages_the_downloaded_version() {
    let server = remote_server(&[("app", "1.0.0")]);
    let source = empty_source();
    let updater = updater(&source, &server.base_url());

    updater
      .download(&[bundle_update("app", "1.0.0")], None)
      .await
      .unwrap();

    assert_eq!(
      source
        .get_remote_staged_version("app")
        .await
        .unwrap()
        .as_deref(),
      Some("1.0.0")
    );
  }

  #[tokio::test]
  async fn download_reports_the_result_of_each_bundle() {
    let server = remote_server(&[("app", "1.0.0")]);
    let source = empty_source();
    let updater = updater(&source, &server.base_url());

    let results = updater
      .download(
        &[
          bundle_update("app", "1.0.0"),
          bundle_update("admin", "0.1.0"),
        ],
        None,
      )
      .await
      .unwrap();

    assert!(download_result(&results, "app").is_ok());
    assert_eq!(
      error_code(download_result(&results, "admin")),
      ErrorCode::RemoteHttp
    );
  }

  #[tokio::test]
  async fn download_does_not_stage_a_bundle_it_failed_to_download() {
    let server = remote_server(&[("app", "1.0.0")]);
    let source = empty_source();
    let updater = updater(&source, &server.base_url());

    updater
      .download(&[bundle_update("admin", "0.1.0")], None)
      .await
      .unwrap();

    assert_eq!(
      source.get_remote_staged_version("admin").await.unwrap(),
      None
    );
  }

  #[tokio::test]
  async fn download_uses_the_download_url_of_the_update() {
    let server = remote_server(&[("app", "1.0.0")]);
    let source = empty_source();
    let updater = updater(&source, OFFLINE_URL);
    let mut update = bundle_update("app", "1.0.0");
    update.download_url = Some(format!("{}/bundles/app/1.0.0", server.base_url()));

    let results = updater.download(&[update], None).await.unwrap();

    assert!(download_result(&results, "app").is_ok());
  }

  #[tokio::test]
  async fn install_activates_the_staged_version() {
    let source = staged_source(&[("app", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);

    let results = updater
      .install(&[install_target("app", "1.0.0", None)])
      .await
      .unwrap();

    assert!(install_result(&results, "app").is_ok());
    assert_eq!(
      current_version(&source, "app").await.as_deref(),
      Some("1.0.0")
    );
  }

  #[tokio::test]
  async fn install_rejects_a_version_which_is_not_staged() {
    let source = staged_source(&[("app", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);

    let results = updater
      .install(&[install_target("app", "9.9.9", None)])
      .await
      .unwrap();

    assert_eq!(
      error_code(install_result(&results, "app")),
      ErrorCode::BundleEntryNotExists
    );
    assert_eq!(current_version(&source, "app").await, None);
  }

  #[tokio::test]
  async fn install_rejects_a_file_which_is_not_a_bundle() {
    let source = staged_source(&[("app", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);
    let filepath = source.get_remote_bundle_filepath("app", "1.0.0").unwrap();
    tokio::fs::write(&filepath, b"not a bundle").await.unwrap();

    let results = updater
      .install(&[install_target("app", "1.0.0", None)])
      .await
      .unwrap();

    assert_eq!(
      error_code(install_result(&results, "app")),
      ErrorCode::InvalidMagicNum
    );
  }

  #[cfg(feature = "integrity")]
  #[tokio::test]
  async fn install_rejects_a_bundle_which_does_not_match_its_integrity() {
    use crate::BundleEntry;
    use crate::integrity::IntegrityAlgorithm;

    let mut builder = TestingSourceBuilder::new();
    builder.set_integrity_algorithm(IntegrityAlgorithm::Sha256);
    builder.add_remote_bundle(TestingBundle::new("app", "1.0.0"));
    builder.set_remote_staged_version("app", "1.0.0");
    let source = Arc::new(builder.build().unwrap());
    let updater = updater(&source, OFFLINE_URL);

    let mut tampered = TestingBundle::new("app", "1.0.0");
    tampered.add_entry(
      "/index.html",
      BundleEntry::new(b"tampered", "text/html", None),
    );
    let filepath = source.get_remote_bundle_filepath("app", "1.0.0").unwrap();
    tokio::fs::write(&filepath, tampered.make_bundle_data().unwrap())
      .await
      .unwrap();

    let results = updater
      .install(&[install_target("app", "1.0.0", None)])
      .await
      .unwrap();

    assert_eq!(
      error_code(install_result(&results, "app")),
      ErrorCode::IntegrityVerifyFailed
    );
  }

  #[tokio::test]
  async fn install_fails_every_target_of_an_atomic_group() {
    let source = staged_source(&[("app", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);

    let results = updater
      .install(&[
        install_target("app", "1.0.0", Some(&["admin"])),
        install_target("admin", "0.1.0", None),
      ])
      .await
      .unwrap();

    assert_eq!(
      error_code(install_result(&results, "app")),
      ErrorCode::InstallAtomicFailed
    );
    assert!(install_result(&results, "admin").is_err());
    assert_eq!(current_version(&source, "app").await, None);
  }

  #[tokio::test]
  async fn install_activates_the_targets_outside_a_failed_group() {
    let source = staged_source(&[("app", "1.0.0"), ("docs", "3.0.0")]);
    let updater = updater(&source, OFFLINE_URL);

    let results = updater
      .install(&[
        install_target("app", "1.0.0", Some(&["admin"])),
        install_target("admin", "0.1.0", None),
        install_target("docs", "3.0.0", None),
      ])
      .await
      .unwrap();

    assert!(install_result(&results, "docs").is_ok());
    assert_eq!(
      current_version(&source, "docs").await.as_deref(),
      Some("3.0.0")
    );
    assert_eq!(current_version(&source, "app").await, None);
  }
}
