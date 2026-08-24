use crate::remote::{BundleUpdate, Remote, RemoteGetUpdateOptions, RemoteUpdateResponse, Update};
use crate::source::{
  ManifestBundleItemStatus, ManifestSetCurrentVersionResultKind, ManifestStageData,
  ManifestVersionData, Source, SourceKind, SourceListItem,
};
use crate::updater::tmp_file::TmpFile;
use crate::updater::update_file::UpdateFile;
use crate::updater::{
  UpdaterDownloadOptions, UpdaterDownloadResult, UpdaterDownloadResultInner,
  UpdaterGetUpdateOptions, UpdaterInstallResult, UpdaterInstallResultKind, UpdaterInstallTarget,
  UpdaterOptions, UpdaterRollbackResult, UpdaterRollbackResultKind, UpdaterRollbackTarget,
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
use time::OffsetDateTime;
use time::format_description::well_known::{Iso8601, Rfc3339};
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Default, Clone)]
pub struct UpdaterBuilder {
  source: Option<Arc<Source>>,
  remote: Option<Arc<Remote>>,
  update_filepath: Option<PathBuf>,
  options: UpdaterOptions,
}

impl UpdaterBuilder {
  #[must_use]
  pub fn source(mut self, source: impl Into<Arc<Source>>) -> Self {
    self.source = Some(source.into());
    self
  }

  #[must_use]
  pub fn remote(mut self, remote: impl Into<Arc<Remote>>) -> Self {
    self.remote = Some(remote.into());
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
  source: Arc<Source>,
  remote: Arc<Remote>,
  options: UpdaterOptions,
  file: UpdateFile,
  // Jobs that result in actual changes to the file on disk acquire a lock to ensure they are
  // executed serially.
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
          .keys
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

    // An update older than the stored one would take the client back to a version it has
    // already moved past, so it is refused and what is stored stays in place.
    let next = self
      .remote
      .get_update(Some(opts))
      .await?
      .filter(|next| prev.as_ref().is_none_or(|prev| !is_rollback(next, prev)));

    let resp = match next {
      Some(next) => {
        let changed = prev
          .as_ref()
          .is_none_or(|prev| prev.update.id != next.update.id);
        if changed {
          self.file.write(&next).await?;
        }
        next
      }
      // Not modified, or refused: whatever was stored is still the current update.
      None => match prev {
        Some(prev) => prev,
        None => return Ok(None),
      },
    };

    if resp.update.runtime_version > crate::RUNTIME_VERSION {
      return Ok(None);
    }

    let locals = self.source.list_bundles().await?;
    let current_versions = current_versions(&locals);
    let bundles = resp
      .update
      .bundles
      .iter()
      .filter(|bundle| {
        current_versions
          .get(bundle.name.as_str())
          .is_none_or(|current| *current != bundle.version)
      })
      .cloned()
      .collect::<Vec<_>>();
    if bundles.is_empty() {
      return Ok(None);
    }

    Ok(Some(Update {
      bundles,
      ..resp.update
    }))
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

    let jobs = bundle_updates.to_vec();
    let results = futures_util::stream::iter(jobs.into_iter().map(|bundle_update| {
      let cancellation = cancellation.clone();
      async move {
        let result = self.download_one(seq, &bundle_update, cancellation).await;
        Ok::<UpdaterDownloadResultInner, crate::Error>(UpdaterDownloadResultInner {
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
          ManifestStageData {
            version: x.update.version.to_owned(),
            data: Some(ManifestVersionData {
              integrity: x.update.integrity.to_owned(),
              metadata: x.update.metadata.to_owned(),
            }),
          },
        )
      })
      .collect::<HashMap<_, _>>();
    self.source.stage_remote_bundles(staged).await?;

    Ok(results.into_iter().map(Into::into).collect::<Vec<_>>())
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

  pub async fn install(
    &self,
    targets: &[UpdaterInstallTarget],
  ) -> crate::Result<Vec<UpdaterInstallResult>> {
    let _guard = self.lock(&None).await?;

    let mut outcomes = Vec::with_capacity(targets.len());
    for target in targets {
      outcomes.push(self.install_one(target).await);
    }

    let versions = targets
      .iter()
      .zip(&outcomes)
      .filter_map(|(target, outcome)| {
        Some((target.name.to_owned(), outcome.as_ref().ok()?.to_owned()))
      })
      .collect::<HashMap<_, _>>();
    let activated = self.activate(versions).await?;

    let results = targets
      .iter()
      .zip(outcomes)
      .map(|(target, outcome)| {
        let (install_version, result) = match outcome {
          Ok(version) => match activated.get(&target.name) {
            Some(ManifestSetCurrentVersionResultKind::Settled) => {
              (Some(version), UpdaterInstallResultKind::Installed)
            }
            Some(ManifestSetCurrentVersionResultKind::VersionNotExists) => {
              (None, UpdaterInstallResultKind::StagedVersionNotMatched)
            }
            _ => (None, UpdaterInstallResultKind::StagedBundleNotExists),
          },
          Err(kind) => (None, kind),
        };
        UpdaterInstallResult {
          name: target.name.to_owned(),
          target_version: target.version.to_owned(),
          install_version,
          result,
        }
      })
      .collect();

    Ok(results)
  }

  async fn install_one(
    &self,
    target: &UpdaterInstallTarget,
  ) -> Result<String, UpdaterInstallResultKind> {
    let Some(staged) = self.source.get_remote_staged_version(&target.name).await? else {
      return Err(UpdaterInstallResultKind::StagedBundleNotExists);
    };
    if target.version.as_ref().is_some_and(|x| x != &staged) {
      return Err(UpdaterInstallResultKind::StagedVersionNotMatched);
    }
    self.verify_bundle(&target.name, &staged).await?;
    Ok(staged)
  }

  pub async fn rollback(
    &self,
    targets: &[UpdaterRollbackTarget],
  ) -> crate::Result<Vec<UpdaterRollbackResult>> {
    let _guard = self.lock(&None).await?;

    let mut outcomes = Vec::with_capacity(targets.len());
    for target in targets {
      outcomes.push(self.rollback_one(target).await);
    }

    let versions = targets
      .iter()
      .zip(&outcomes)
      .filter_map(|(target, outcome)| {
        Some((target.name.to_owned(), outcome.as_ref().ok()?.to_owned()))
      })
      .collect::<HashMap<_, _>>();
    let activated = self.activate(versions).await?;

    let results = targets
      .iter()
      .zip(outcomes)
      .map(|(target, outcome)| {
        let (rollback_version, result) = match outcome {
          Ok(version) => match activated.get(&target.name) {
            Some(ManifestSetCurrentVersionResultKind::Settled) => {
              (Some(version), UpdaterRollbackResultKind::RolledBack)
            }
            Some(ManifestSetCurrentVersionResultKind::VersionNotExists) => {
              (None, UpdaterRollbackResultKind::PreviousVersionNotMatched)
            }
            _ => (None, UpdaterRollbackResultKind::PreviousBundleNotExists),
          },
          Err(kind) => (None, kind),
        };
        UpdaterRollbackResult {
          name: target.name.to_owned(),
          target_version: target.version.to_owned(),
          rollback_version,
          result,
        }
      })
      .collect();

    Ok(results)
  }

  async fn rollback_one(
    &self,
    target: &UpdaterRollbackTarget,
  ) -> Result<String, UpdaterRollbackResultKind> {
    let Some(previous) = self
      .source
      .get_remote_previous_version(&target.name)
      .await?
    else {
      return Err(UpdaterRollbackResultKind::PreviousBundleNotExists);
    };
    if target.version.as_ref().is_some_and(|x| x != &previous) {
      return Err(UpdaterRollbackResultKind::PreviousVersionNotMatched);
    }
    self.verify_bundle(&target.name, &previous).await?;
    Ok(previous)
  }

  async fn activate(
    &self,
    versions: HashMap<String, String>,
  ) -> crate::Result<HashMap<String, ManifestSetCurrentVersionResultKind>> {
    let results = self.source.update_remote_versions(versions).await?;

    let bundle_names = results
      .iter()
      .filter(|x| x.kind == ManifestSetCurrentVersionResultKind::Settled)
      .map(|x| {
        self.source.unload(&x.name);
        x.name.to_owned()
      })
      .collect::<Vec<_>>();
    let _ = self.source.prune_remote_bundles(&bundle_names).await;

    Ok(results.into_iter().map(|x| (x.name, x.kind)).collect())
  }

  async fn verify_bundle(&self, bundle_name: &str, version: &str) -> Result<(), VerifyFailure> {
    let filepath = self
      .source
      .get_remote_bundle_filepath(bundle_name, version)?;
    let data = match tokio::fs::read(&filepath).await {
      Ok(data) => data,
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(VerifyFailure::NotExists),
      Err(e) => return Err(VerifyFailure::Error(e.into())),
    };

    // Read bundle data to expect the file is webview bundle formatted.
    Reader::<BundleDescriptor>::read(&mut BundleReader::new(Cursor::new(&data)))
      .map_err(|_| VerifyFailure::Failed)?;

    #[cfg(feature = "integrity")]
    {
      let Some(version_data) = self
        .source
        .get_remote_version_data(bundle_name, version)
        .await?
      else {
        return Err(VerifyFailure::NotExists);
      };
      crate::integrity::verify_integrity(
        &self.options.integrity.policy,
        version_data.integrity.as_deref(),
        &data,
      )
      .map_err(|_| VerifyFailure::Failed)?;
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

enum VerifyFailure {
  NotExists,
  Failed,
  Error(crate::Error),
}

impl From<crate::Error> for VerifyFailure {
  fn from(error: crate::Error) -> Self {
    Self::Error(error)
  }
}

impl From<VerifyFailure> for UpdaterInstallResultKind {
  fn from(failure: VerifyFailure) -> Self {
    match failure {
      VerifyFailure::NotExists => Self::StagedBundleNotExists,
      VerifyFailure::Failed => Self::VerifyFailed,
      VerifyFailure::Error(error) => Self::Error(error),
    }
  }
}

impl From<VerifyFailure> for UpdaterRollbackResultKind {
  fn from(failure: VerifyFailure) -> Self {
    match failure {
      VerifyFailure::NotExists => Self::PreviousBundleNotExists,
      VerifyFailure::Failed => Self::VerifyFailed,
      VerifyFailure::Error(error) => Self::Error(error),
    }
  }
}

/// Whether `next` was created before the update already stored as `prev`.
///
/// A `created_at` which cannot be read as a datetime counts as a rollback when it comes from
/// the server, so a malformed field is not a way around this check, while an unreadable stored
/// one does not lock the client out of updating for good.
fn is_rollback(next: &RemoteUpdateResponse, prev: &RemoteUpdateResponse) -> bool {
  let Ok(next) = parse_datetime(&next.update.created_at) else {
    return true;
  };
  let Ok(prev) = parse_datetime(&prev.update.created_at) else {
    return false;
  };
  next < prev
}

fn parse_datetime(value: &str) -> Result<OffsetDateTime, time::error::Parse> {
  OffsetDateTime::parse(value, &Rfc3339)
    .or_else(|_| OffsetDateTime::parse(value, &Iso8601::DEFAULT))
}

fn current_versions(items: &[SourceListItem]) -> HashMap<&str, &str> {
  let mut versions: HashMap<&str, &str> = HashMap::new();
  for item in items
    .iter()
    .filter(|x| x.item.status == ManifestBundleItemStatus::Current)
  {
    match item.source {
      SourceKind::Remote => {
        versions.insert(&item.item.name, &item.item.version);
      }
      SourceKind::Builtin => {
        versions
          .entry(&item.item.name)
          .or_insert(&item.item.version);
      }
    }
  }
  versions
}

#[cfg(all(test, feature = "testing"))]
mod tests {
  use super::*;
  use crate::ErrorCode;
  use crate::remote::RemoteUpdateResponse;
  use crate::testing::{TempDir, TestingBundle, TestingRemoteServer, TestingSourceBuilder};
  use crate::updater::UpdaterDownloadResultKind;

  const OFFLINE_URL: &str = "http://127.0.0.1:1";

  fn update_filepath() -> PathBuf {
    TempDir::new().dir().join("update.json")
  }

  fn build_updater(
    source: &Arc<Source>,
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

  fn updater(source: &Arc<Source>, base_url: &str) -> Updater {
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

  fn builtin_source(bundles: &[(&str, &str)]) -> Arc<Source> {
    let mut builder = TestingSourceBuilder::new();
    for (name, version) in bundles {
      builder.add_builtin_bundle(TestingBundle::new(*name, *version));
    }
    Arc::new(builder.build().unwrap())
  }

  fn empty_source() -> Arc<Source> {
    Arc::new(TestingSourceBuilder::new().build().unwrap())
  }

  fn staged_source(bundles: &[(&str, &str)]) -> Arc<Source> {
    let mut builder = TestingSourceBuilder::new();
    for (name, version) in bundles {
      builder.add_remote_bundle(TestingBundle::new(*name, *version));
      builder.set_remote_staged_version(*name, *version);
    }
    Arc::new(builder.build().unwrap())
  }

  /// A source serving `current`, with `previous` recorded as the version it was serving
  /// before that.
  fn previous_source(bundles: &[(&str, &str, &str)]) -> Arc<Source> {
    let mut builder = TestingSourceBuilder::new();
    for (name, current, previous) in bundles {
      builder.add_remote_bundle(TestingBundle::new(*name, *current));
      builder.add_remote_bundle(TestingBundle::new(*name, *previous));
      builder.set_remote_current_version(*name, *current);
      builder.set_remote_previous_version(*name, *previous);
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

  /// Rewrites the stored update, so a test can put it in a state the server would not send.
  async fn rewrite_stored_update(filepath: &Path, rewrite: impl FnOnce(&mut RemoteUpdateResponse)) {
    let raw = tokio::fs::read(filepath).await.unwrap();
    let mut stored = serde_json::from_slice::<RemoteUpdateResponse>(&raw).unwrap();
    rewrite(&mut stored);
    tokio::fs::write(filepath, serde_json::to_vec(&stored).unwrap())
      .await
      .unwrap();
  }

  fn install_target(name: &str, version: Option<&str>) -> UpdaterInstallTarget {
    UpdaterInstallTarget {
      name: name.to_owned(),
      version: version.map(|x| x.to_owned()),
    }
  }

  fn rollback_target(name: &str, version: Option<&str>) -> UpdaterRollbackTarget {
    UpdaterRollbackTarget {
      name: name.to_owned(),
      version: version.map(|x| x.to_owned()),
    }
  }

  fn updated_bundles(update: &Update) -> Vec<(&str, &str)> {
    update
      .bundles
      .iter()
      .map(|x| (x.name.as_str(), x.version.as_str()))
      .collect()
  }

  async fn current_version(source: &Source, bundle_name: &str) -> Option<String> {
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
  ) -> &'a UpdaterDownloadResultKind {
    &results
      .iter()
      .find(|x| x.name == bundle_name)
      .unwrap()
      .result
  }

  #[track_caller]
  fn install_result<'a>(
    results: &'a [UpdaterInstallResult],
    bundle_name: &str,
  ) -> &'a UpdaterInstallResultKind {
    &results
      .iter()
      .find(|x| x.name == bundle_name)
      .unwrap()
      .result
  }

  #[track_caller]
  fn rollback_result<'a>(
    results: &'a [UpdaterRollbackResult],
    bundle_name: &str,
  ) -> &'a UpdaterRollbackResultKind {
    &results
      .iter()
      .find(|x| x.name == bundle_name)
      .unwrap()
      .result
  }

  #[track_caller]
  fn error_code(result: &UpdaterDownloadResultKind) -> ErrorCode {
    match result {
      UpdaterDownloadResultKind::Error(error) => error.code(),
      UpdaterDownloadResultKind::Downloaded => panic!("the bundle was downloaded"),
    }
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

    rewrite_stored_update(&filepath, |stored| {
      stored.update.bundles = vec![bundle_update("app", "1.2.0")];
    })
    .await;

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
  async fn get_update_refuses_an_update_created_before_the_stored_one() {
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

    // Without an etag the server answers with the update it serves, which was created before
    // the one stored here.
    rewrite_stored_update(&filepath, |stored| {
      stored.etag = None;
      stored.update.created_at = "2030-01-01T00:00:00Z".to_owned();
      stored.update.bundles = vec![bundle_update("app", "1.2.0")];
    })
    .await;

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
    let stored = tokio::fs::read(&filepath).await.unwrap();
    let stored = serde_json::from_slice::<RemoteUpdateResponse>(&stored).unwrap();
    assert_eq!(
      stored.update.created_at, "2030-01-01T00:00:00Z",
      "a refused update must not replace what is stored"
    );
  }

  #[tokio::test]
  async fn get_update_takes_an_update_created_after_the_stored_one() {
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

    rewrite_stored_update(&filepath, |stored| {
      stored.etag = None;
      stored.update.created_at = "2000-01-01T00:00:00Z".to_owned();
      stored.update.bundles = vec![bundle_update("app", "1.2.0")];
    })
    .await;

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

    assert_eq!(updated_bundles(&update), vec![("app", "1.1.0")]);
  }

  #[test]
  fn a_rollback_is_read_from_the_creation_times() {
    let response = |created_at: &str| RemoteUpdateResponse {
      update: Update {
        id: "update".to_owned(),
        created_at: created_at.to_owned(),
        runtime_version: crate::RUNTIME_VERSION,
        bundles: vec![],
        metadata: HashMap::new(),
      },
      etag: None,
      signature: None,
    };
    let stored = response("2026-01-01T00:00:00Z");

    assert!(is_rollback(&response("2025-12-31T23:59:59Z"), &stored));
    assert!(!is_rollback(&response("2026-01-01T00:00:01Z"), &stored));
    assert!(!is_rollback(&response("2026-01-01T00:00:00Z"), &stored));
    // The same instant written with an offset, and the ISO 8601 basic format.
    assert!(!is_rollback(
      &response("2026-01-01T09:00:00+09:00"),
      &stored
    ));
    assert!(is_rollback(&response("20251231T235959Z"), &stored));
    // A creation time the server made unreadable is refused, one already stored is not.
    assert!(is_rollback(&response("not a datetime"), &stored));
    assert!(!is_rollback(&stored, &response("not a datetime")));
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
      UpdaterSignatureOptions::default().add_key(server.signature_key_set("default").unwrap()),
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

    assert!(matches!(
      download_result(&results, "app"),
      UpdaterDownloadResultKind::Downloaded
    ));
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

    assert!(matches!(
      download_result(&results, "app"),
      UpdaterDownloadResultKind::Downloaded
    ));
  }

  #[tokio::test]
  async fn install_activates_the_staged_version() {
    let source = staged_source(&[("app", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);

    let results = updater
      .install(&[install_target("app", Some("1.0.0"))])
      .await
      .unwrap();

    assert!(matches!(
      install_result(&results, "app"),
      UpdaterInstallResultKind::Installed
    ));
    assert_eq!(results[0].target_version.as_deref(), Some("1.0.0"));
    assert_eq!(results[0].install_version.as_deref(), Some("1.0.0"));
    assert_eq!(
      current_version(&source, "app").await.as_deref(),
      Some("1.0.0")
    );
  }

  #[tokio::test]
  async fn install_falls_back_to_the_staged_version_of_the_manifest() {
    let source = staged_source(&[("app", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);

    let results = updater
      .install(&[install_target("app", None)])
      .await
      .unwrap();

    assert!(matches!(
      install_result(&results, "app"),
      UpdaterInstallResultKind::Installed
    ));
    assert_eq!(results[0].target_version, None);
    assert_eq!(results[0].install_version.as_deref(), Some("1.0.0"));
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
      .install(&[install_target("app", Some("9.9.9"))])
      .await
      .unwrap();

    assert!(matches!(
      install_result(&results, "app"),
      UpdaterInstallResultKind::StagedVersionNotMatched
    ));
    assert_eq!(results[0].target_version.as_deref(), Some("9.9.9"));
    assert_eq!(results[0].install_version, None);
    assert_eq!(current_version(&source, "app").await, None);
  }

  #[tokio::test]
  async fn install_reports_a_bundle_which_has_nothing_staged() {
    let source = staged_source(&[("app", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);

    let results = updater
      .install(&[install_target("admin", None)])
      .await
      .unwrap();

    assert!(matches!(
      install_result(&results, "admin"),
      UpdaterInstallResultKind::StagedBundleNotExists
    ));
  }

  #[tokio::test]
  async fn install_rejects_a_file_which_is_not_a_bundle() {
    let source = staged_source(&[("app", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);
    let filepath = source.get_remote_bundle_filepath("app", "1.0.0").unwrap();
    tokio::fs::write(&filepath, b"not a bundle").await.unwrap();

    let results = updater
      .install(&[install_target("app", Some("1.0.0"))])
      .await
      .unwrap();

    assert!(matches!(
      install_result(&results, "app"),
      UpdaterInstallResultKind::VerifyFailed
    ));
    assert_eq!(current_version(&source, "app").await, None);
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
      .install(&[install_target("app", Some("1.0.0"))])
      .await
      .unwrap();

    assert!(matches!(
      install_result(&results, "app"),
      UpdaterInstallResultKind::VerifyFailed
    ));
  }

  #[tokio::test]
  async fn install_activates_the_targets_which_did_not_fail() {
    let source = staged_source(&[("app", "1.0.0"), ("docs", "3.0.0")]);
    let updater = updater(&source, OFFLINE_URL);

    let results = updater
      .install(&[
        install_target("app", Some("1.0.0")),
        install_target("admin", Some("0.1.0")),
        install_target("docs", Some("3.0.0")),
      ])
      .await
      .unwrap();

    assert!(matches!(
      install_result(&results, "admin"),
      UpdaterInstallResultKind::StagedBundleNotExists
    ));
    assert!(matches!(
      install_result(&results, "app"),
      UpdaterInstallResultKind::Installed
    ));
    assert!(matches!(
      install_result(&results, "docs"),
      UpdaterInstallResultKind::Installed
    ));
    assert_eq!(
      current_version(&source, "app").await.as_deref(),
      Some("1.0.0")
    );
    assert_eq!(
      current_version(&source, "docs").await.as_deref(),
      Some("3.0.0")
    );
  }

  #[tokio::test]
  async fn rollback_activates_the_previous_version() {
    let source = previous_source(&[("app", "1.1.0", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);

    let results = updater
      .rollback(&[rollback_target("app", Some("1.0.0"))])
      .await
      .unwrap();

    assert!(matches!(
      rollback_result(&results, "app"),
      UpdaterRollbackResultKind::RolledBack
    ));
    assert_eq!(results[0].target_version.as_deref(), Some("1.0.0"));
    assert_eq!(results[0].rollback_version.as_deref(), Some("1.0.0"));
    assert_eq!(
      current_version(&source, "app").await.as_deref(),
      Some("1.0.0")
    );
    assert_eq!(
      source.get_remote_previous_version("app").await.unwrap(),
      None
    );
  }

  #[tokio::test]
  async fn rollback_falls_back_to_the_previous_version_of_the_manifest() {
    let source = previous_source(&[("app", "1.1.0", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);

    let results = updater
      .rollback(&[rollback_target("app", None)])
      .await
      .unwrap();

    assert!(matches!(
      rollback_result(&results, "app"),
      UpdaterRollbackResultKind::RolledBack
    ));
    assert_eq!(results[0].target_version, None);
    assert_eq!(results[0].rollback_version.as_deref(), Some("1.0.0"));
    assert_eq!(
      current_version(&source, "app").await.as_deref(),
      Some("1.0.0")
    );
  }

  #[tokio::test]
  async fn rollback_rejects_a_version_which_is_not_the_previous_one() {
    let source = previous_source(&[("app", "1.1.0", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);

    let results = updater
      .rollback(&[rollback_target("app", Some("0.9.0"))])
      .await
      .unwrap();

    assert!(matches!(
      rollback_result(&results, "app"),
      UpdaterRollbackResultKind::PreviousVersionNotMatched
    ));
    assert_eq!(results[0].target_version.as_deref(), Some("0.9.0"));
    assert_eq!(results[0].rollback_version, None);
    assert_eq!(
      current_version(&source, "app").await.as_deref(),
      Some("1.1.0")
    );
  }

  #[tokio::test]
  async fn rollback_reports_a_bundle_which_has_no_previous_version() {
    let source = staged_source(&[("app", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);

    let results = updater
      .rollback(&[rollback_target("app", None)])
      .await
      .unwrap();

    assert!(matches!(
      rollback_result(&results, "app"),
      UpdaterRollbackResultKind::PreviousBundleNotExists
    ));
  }

  #[tokio::test]
  async fn rollback_rejects_a_file_which_is_not_a_bundle() {
    let source = previous_source(&[("app", "1.1.0", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);
    let filepath = source.get_remote_bundle_filepath("app", "1.0.0").unwrap();
    tokio::fs::write(&filepath, b"not a bundle").await.unwrap();

    let results = updater
      .rollback(&[rollback_target("app", None)])
      .await
      .unwrap();

    assert!(matches!(
      rollback_result(&results, "app"),
      UpdaterRollbackResultKind::VerifyFailed
    ));
    assert_eq!(
      current_version(&source, "app").await.as_deref(),
      Some("1.1.0")
    );
  }

  #[tokio::test]
  async fn rollback_activates_the_targets_which_did_not_fail() {
    let source = previous_source(&[("app", "1.1.0", "1.0.0")]);
    let updater = updater(&source, OFFLINE_URL);

    let results = updater
      .rollback(&[rollback_target("app", None), rollback_target("admin", None)])
      .await
      .unwrap();

    assert!(matches!(
      rollback_result(&results, "admin"),
      UpdaterRollbackResultKind::PreviousBundleNotExists
    ));
    assert!(matches!(
      rollback_result(&results, "app"),
      UpdaterRollbackResultKind::RolledBack
    ));
    assert_eq!(
      current_version(&source, "app").await.as_deref(),
      Some("1.0.0")
    );
  }
}
