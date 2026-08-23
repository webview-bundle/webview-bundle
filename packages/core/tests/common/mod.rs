#![allow(dead_code)]

use http::Request;
use std::sync::Arc;
use wvb::BundleEntry;
use wvb::remote::{BundleUpdate, Remote, Update};
use wvb::source::{Source, SourceOptions};
use wvb::testing::{TempDir, TestingBundle, TestingRemoteServer, TestingSourceBuilder};
use wvb::updater::{
  Updater, UpdaterDownloadResultKind, UpdaterInstallResultKind, UpdaterInstallTarget,
  UpdaterOptions,
};

/// A base url nothing listens on, for updaters that must never reach a server.
pub const OFFLINE_URL: &str = "http://127.0.0.1:1";

pub const INDEX: &str = "/index.html";

/// A `GET` request with an empty body.
pub fn get(uri: &str) -> Request<Vec<u8>> {
  Request::builder()
    .uri(uri)
    .method("GET")
    .body(vec![])
    .unwrap()
}

/// A bundle serving `body` at `/index.html`.
pub fn bundle(name: &str, version: &str, body: &[u8]) -> TestingBundle {
  bundle_of(name, version, &[(INDEX, body)])
}

pub fn bundle_of(name: &str, version: &str, entries: &[(&str, &[u8])]) -> TestingBundle {
  let mut bundle = TestingBundle::new(name, version);
  for (path, data) in entries {
    bundle.add_entry(*path, BundleEntry::new(data, "text/html", None));
  }
  bundle
}

/// A source that ships `bundles` as builtin, each of them active.
pub fn builtin_source(bundles: Vec<TestingBundle>) -> Arc<Source> {
  let mut builder = TestingSourceBuilder::new();
  for bundle in bundles {
    builder.add_builtin_bundle(bundle);
  }
  Arc::new(builder.build().unwrap())
}

pub struct SourceDirs {
  pub builtin: std::path::PathBuf,
  pub remote: std::path::PathBuf,
}

/// Same as [`builtin_source`], keeping the directories so the source can be built again.
pub fn source_with_dirs(bundles: Vec<TestingBundle>) -> (Arc<Source>, SourceDirs) {
  let mut builder = TestingSourceBuilder::new();
  for bundle in bundles {
    builder.add_builtin_bundle(bundle);
  }
  let dirs = SourceDirs {
    builtin: builder.builtin_dir().to_path_buf(),
    remote: builder.remote_dir().to_path_buf(),
  };
  (Arc::new(builder.build().unwrap()), dirs)
}

/// A source over the same directories, as a restarted app would build it.
pub fn reload(dirs: &SourceDirs) -> Arc<Source> {
  reload_with(dirs, SourceOptions::default())
}

pub fn reload_with(dirs: &SourceDirs, options: SourceOptions) -> Arc<Source> {
  Arc::new(
    Source::builder()
      .builtin_dir(&dirs.builtin)
      .remote_dir(&dirs.remote)
      .options(options)
      .build(),
  )
}

/// A server serving `bundles`, each of them as the current version of its bundle.
pub fn remote_server(bundles: Vec<TestingBundle>) -> TestingRemoteServer {
  let mut server = TestingRemoteServer::new();
  for bundle in bundles {
    let (name, version) = (bundle.name().to_owned(), bundle.version().to_owned());
    server.insert_bundle(bundle);
    server.set_current_version(name, version);
  }
  server
}

pub fn updater(source: &Arc<Source>, base_url: &str) -> Updater {
  updater_with(source, base_url, UpdaterOptions::default())
}

pub fn updater_with(source: &Arc<Source>, base_url: &str, options: UpdaterOptions) -> Updater {
  Updater::builder()
    .source(source.clone())
    .remote(Arc::new(
      Remote::builder().base_url(base_url).build().unwrap(),
    ))
    .update_filepath(&TempDir::new().dir().join("update.json"))
    .options(options)
    .build()
    .unwrap()
}

pub fn target(name: &str, version: &str) -> UpdaterInstallTarget {
  UpdaterInstallTarget {
    name: name.to_owned(),
    version: Some(version.to_owned()),
  }
}

/// Panics unless `result` says the bundle was installed, naming what happened instead.
#[track_caller]
pub fn expect_installed(bundle_name: &str, result: &UpdaterInstallResultKind) {
  if !matches!(result, UpdaterInstallResultKind::Installed) {
    panic!("install of {bundle_name} failed: {result:?}");
  }
}

pub fn bundle_update(name: &str, version: &str) -> BundleUpdate {
  BundleUpdate {
    name: name.to_owned(),
    version: version.to_owned(),
    download_url: None,
    integrity: None,
    metadata: None,
  }
}

pub async fn fetch_update(updater: &Updater) -> Update {
  updater
    .get_update(None)
    .await
    .unwrap()
    .expect("an update must be available")
}

pub async fn download_all(updater: &Updater, bundles: &[BundleUpdate]) {
  for item in updater.download(bundles, None).await.unwrap() {
    if let UpdaterDownloadResultKind::Error(e) = item.result {
      panic!("download of {} failed: {e}", item.name);
    }
  }
}

pub async fn install_all(updater: &Updater, bundles: &[BundleUpdate]) {
  let targets = bundles
    .iter()
    .map(|x| target(&x.name, &x.version))
    .collect::<Vec<_>>();
  for item in updater.install(&targets).await.unwrap() {
    expect_installed(&item.name, &item.result);
  }
}

/// Runs a whole update round: fetch, download every bundle it names, then activate them.
pub async fn update_all(updater: &Updater) -> Vec<(String, String)> {
  let update = fetch_update(updater).await;
  download_all(updater, &update.bundles).await;
  install_all(updater, &update.bundles).await;
  update
    .bundles
    .iter()
    .map(|x| (x.name.to_owned(), x.version.to_owned()))
    .collect()
}

/// Rewrites a bundle file in place, leaving its manifest entry untouched.
pub fn corrupt(filepath: &std::path::Path, corrupt: impl FnOnce(&mut [u8])) {
  let mut data = std::fs::read(filepath).unwrap();
  corrupt(&mut data);
  std::fs::write(filepath, data).unwrap();
}
