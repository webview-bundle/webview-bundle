use crate::remote::Remote;
use crate::source::{
  BundleManifestData, BundleManifestEntry, BundleManifestMetadata, BundleSource,
};
use crate::testing::TempDir;
use crate::{Bundle, BundleEntry, BundleWriter, Writer};
use httpmock::{HttpMockRequest, HttpMockResponse, MockExt, MockServer};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct MockBundle {
  name: String,
  version: String,
  etag: Option<String>,
  integrity: Option<String>,
  signature: Option<String>,
  last_modified: Option<String>,
  entries: HashMap<String, BundleEntry>,
}

impl From<(String, String)> for MockBundle {
  fn from(value: (String, String)) -> Self {
    Self::new(value.0, value.1)
  }
}

impl Hash for MockBundle {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.name.hash(state);
    self.version.hash(state);
  }
}

impl PartialEq for MockBundle {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name && self.version == other.version
  }
}

impl Eq for MockBundle {}

impl MockBundle {
  pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      etag: None,
      integrity: None,
      signature: None,
      last_modified: None,
      entries: HashMap::new(),
    }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn version(&self) -> &str {
    &self.version
  }

  pub fn etag(&self) -> Option<&str> {
    self.etag.as_deref()
  }

  pub fn integrity(&self) -> Option<&str> {
    self.integrity.as_deref()
  }

  pub fn signature(&self) -> Option<&str> {
    self.signature.as_deref()
  }

  pub fn last_modified(&self) -> Option<&str> {
    self.last_modified.as_deref()
  }

  pub fn remote_headers(&self) -> Vec<(String, String)> {
    let mut headers = vec![];
    headers.push(("webview-bundle-name".to_owned(), self.name.to_owned()));
    headers.push(("webview-bundle-version".to_owned(), self.version.to_owned()));
    if let Some(etag) = &self.etag {
      headers.push(("etag".to_owned(), etag.to_owned()));
    }
    if let Some(integrity) = &self.integrity {
      headers.push(("webview-bundle-integrity".to_owned(), integrity.to_owned()));
    }
    if let Some(signature) = &self.signature {
      headers.push(("webview-bundle-signature".to_owned(), signature.to_owned()));
    }
    if let Some(last_modified) = &self.last_modified {
      headers.push(("last-modified".to_owned(), last_modified.to_owned()));
    }
    headers
  }

  pub fn with_entry(mut self, path: impl Into<String>, entry: BundleEntry) -> Self {
    self.entries.insert(path.into(), entry);
    self
  }

  pub fn add_entry(&mut self, path: impl Into<String>, entry: BundleEntry) -> &mut Self {
    self.entries.insert(path.into(), entry);
    self
  }

  pub fn bundle(&self) -> Bundle {
    let mut builder = Bundle::builder();
    for (path, entry) in self.entries.iter() {
      builder.insert_entry(path, entry.clone());
    }
    builder.build().unwrap()
  }

  pub fn bundle_data(&self) -> Vec<u8> {
    let mut data = vec![];
    let mut writer = BundleWriter::new(Cursor::new(&mut data));

    let bundle = self.bundle();
    writer.write(&bundle).unwrap();

    data
  }

  pub fn metadata(&self) -> BundleManifestMetadata {
    BundleManifestMetadata {
      etag: self.etag.to_owned(),
      integrity: self.integrity.to_owned(),
      signature: self.signature.to_owned(),
      last_modified: self.last_modified.to_owned(),
    }
  }

  pub fn is_same(&self, name: &str, version: &str) -> bool {
    self.name == name && self.version == version
  }
}

#[derive(Default)]
pub struct CurrentVersions {
  versions: HashMap<String, String>,
}

impl CurrentVersions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn set(&mut self, name: impl Into<String>, version: impl Into<String>) -> &mut Self {
    self.versions.insert(name.into(), version.into());
    self
  }

  pub fn unset(&mut self, name: impl Into<String>) -> &mut Self {
    self.versions.remove(&name.into());
    self
  }

  pub fn unset_if_current(
    &mut self,
    name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    let name = name.into();
    let version = version.into();
    if let Some(v) = self.versions.get(&name)
      && v == &version
    {
      self.versions.remove(&name);
    }
    self
  }
}

#[derive(Default)]
pub struct MockBundleCollection {
  bundles: HashSet<MockBundle>,
}

impl MockBundleCollection {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn add(&mut self, bundle: MockBundle) -> &mut Self {
    self.bundles.replace(bundle);
    self
  }

  pub fn remove(&mut self, name: impl Into<String>, version: impl Into<String>) -> &mut Self {
    let name = name.into();
    let version = version.into();
    self.bundles.remove(&(name, version).into());
    self
  }

  pub fn remove_all(&mut self, name: impl Into<String>) -> &mut Self {
    let name = name.into();
    self.bundles.retain(|x| x.name() != &name);
    self
  }

  pub fn clear(&mut self) -> &mut Self {
    self.bundles.clear();
    self
  }
}

pub struct MockSource {
  _temp_dir: TempDir,
  builtin_dir: PathBuf,
  builtin_bundles: MockBundleCollection,
  builtin_current_versions: CurrentVersions,
  remote_dir: PathBuf,
  remote_bundles: MockBundleCollection,
  remote_current_versions: CurrentVersions,
}

impl MockSource {
  pub fn new() -> Self {
    let temp_dir = TempDir::new();
    let builtin_dir = temp_dir.dir().join("source").join("builtin");
    let remote_dir = temp_dir.dir().join("source").join("remote");
    fs::create_dir_all(&builtin_dir).unwrap();
    fs::create_dir_all(&remote_dir).unwrap();
    Self {
      _temp_dir: temp_dir,
      builtin_dir,
      builtin_bundles: MockBundleCollection::new(),
      builtin_current_versions: CurrentVersions::new(),
      remote_dir,
      remote_bundles: MockBundleCollection::new(),
      remote_current_versions: CurrentVersions::new(),
    }
  }

  pub fn get_source(&self) -> BundleSource {
    BundleSource::builder()
      .builtin_dir(&self.builtin_dir)
      .remote_dir(&self.remote_dir)
      .build()
  }

  pub fn add_builtin_bundle(&mut self, bundle: MockBundle) -> &mut Self {
    let filepath = self.builtin_dir.join(bundle.name()).join(format!(
      "{}_{}.wvb",
      bundle.name(),
      bundle.version()
    ));
    fs::create_dir_all(filepath.parent().unwrap()).unwrap();
    fs::write(filepath, bundle.bundle_data()).unwrap();
    self.builtin_bundles.add(bundle);
    self.sync_builtin_manifest();
    self
  }

  pub fn remove_builtin_bundle(
    &mut self,
    name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    let name = name.into();
    let version = version.into();
    let _ = fs::remove_file(self.builtin_bundle_filepath(&name, &version));
    self.builtin_bundles.remove(&name, &version);
    self
      .builtin_current_versions
      .unset_if_current(&name, &version);
    self.sync_builtin_manifest();
    self
  }

  pub fn remove_builtin_bundle_all(&mut self, name: impl Into<String>) -> &mut Self {
    let name = name.into();
    let _ = fs::remove_dir_all(self.builtin_dir.join(&name));
    self.builtin_bundles.remove_all(&name);
    self.builtin_current_versions.unset(&name);
    self.sync_builtin_manifest();
    self
  }

  pub fn set_builtin_current_version(
    &mut self,
    name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    self.builtin_current_versions.set(name, version);
    self.sync_builtin_manifest();
    self
  }

  pub fn add_remote_bundle(&mut self, bundle: MockBundle) -> &mut Self {
    let filepath = self.remote_dir.join(bundle.name()).join(format!(
      "{}_{}.wvb",
      bundle.name(),
      bundle.version()
    ));
    fs::create_dir_all(filepath.parent().unwrap()).unwrap();
    self.remote_bundles.add(bundle);
    self.sync_remote_manifest();
    self
  }

  pub fn remove_remote_bundle(
    &mut self,
    name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    let name = name.into();
    let version = version.into();
    let _ = fs::remove_file(self.remote_bundle_filepath(&name, &version));
    self.remote_bundles.remove(&name, &version);
    self
      .remote_current_versions
      .unset_if_current(&name, &version);
    self.sync_builtin_manifest();
    self
  }

  pub fn remove_remote_bundle_all(&mut self, name: impl Into<String>) -> &mut Self {
    let name = name.into();
    let _ = fs::remove_dir_all(self.remote_dir.join(&name));
    self.remote_bundles.remove_all(&name);
    self.remote_current_versions.unset(&name);
    self.sync_builtin_manifest();
    self
  }

  pub fn set_remote_current_version(
    &mut self,
    name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    self.remote_current_versions.set(name, version);
    self.sync_remote_manifest();
    self
  }

  pub fn sync_manifest(&self) {
    self.sync_builtin_manifest();
    self.sync_remote_manifest();
  }

  fn sync_builtin_manifest(&self) {
    if let Some(manifest) =
      Self::make_manifest(&self.builtin_bundles, &self.builtin_current_versions)
    {
      let filepath = self.builtin_dir.join("manifest.json");
      fs::write(filepath, serde_json::to_string(&manifest).unwrap()).unwrap();
    }
  }

  fn sync_remote_manifest(&self) {
    if let Some(manifest) = Self::make_manifest(&self.remote_bundles, &self.remote_current_versions)
    {
      let filepath = self.remote_dir.join("manifest.json");
      fs::write(filepath, serde_json::to_string(&manifest).unwrap()).unwrap();
    }
  }

  fn builtin_bundle_filepath(&self, name: &str, version: &str) -> PathBuf {
    self
      .builtin_dir
      .join(name)
      .join(Self::bundle_filename(name, version))
  }

  fn remote_bundle_filepath(&self, name: &str, version: &str) -> PathBuf {
    self
      .remote_dir
      .join(name)
      .join(Self::bundle_filename(name, version))
  }

  fn bundle_filename(name: &str, version: &str) -> String {
    format!("{name}_{version}.wvb")
  }

  fn make_manifest(
    collection: &MockBundleCollection,
    versions: &CurrentVersions,
  ) -> Option<BundleManifestData> {
    if collection.bundles.is_empty() {
      return None;
    }
    let mut manifest = BundleManifestData {
      manifest_version: Default::default(),
      entries: HashMap::new(),
    };
    for bundle in collection.bundles.iter() {
      let is_current = versions
        .versions
        .get(bundle.name())
        .is_some_and(|x| x == bundle.version());
      manifest
        .entries
        .entry(bundle.name().to_string())
        .and_modify(|entry| {
          entry
            .versions
            .insert(bundle.version().to_string(), bundle.metadata());
          if is_current {
            entry.current_version = bundle.version().to_string();
          }
        })
        .or_insert_with(|| BundleManifestEntry {
          versions: HashMap::from([(bundle.version().to_string(), bundle.metadata())]),
          current_version: bundle.version().to_string(),
        });
    }
    Some(manifest)
  }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum MockRemoteEndpoint {
  ListBundles,
  GetCurrentInfo,
  Download,
  DownloadVersion,
}

pub struct MockRemote {
  server: MockServer,
  mocks: HashMap<MockRemoteEndpoint, usize>,
  allow_other_versions: bool,
  bundles: Arc<Mutex<MockBundleCollection>>,
  current_versions: Arc<Mutex<CurrentVersions>>,
  channel_bundles: Arc<Mutex<HashMap<String, MockBundleCollection>>>,
  channel_current_versions: Arc<Mutex<HashMap<String, CurrentVersions>>>,
}

impl MockRemote {
  pub fn new() -> Self {
    let server = MockServer::start();
    let mut instance = Self {
      server,
      mocks: HashMap::new(),
      allow_other_versions: false,
      bundles: Arc::new(Mutex::new(MockBundleCollection::new())),
      current_versions: Arc::new(Mutex::new(CurrentVersions::new())),
      channel_bundles: Default::default(),
      channel_current_versions: Default::default(),
    };
    instance.init();
    instance
  }

  pub fn get_remote(&self) -> Remote {
    Remote::builder()
      .endpoint(self.server_url())
      .build()
      .unwrap()
  }

  pub fn server_url(&self) -> String {
    format!("http://{}:{}", self.server.host(), self.server.port())
  }

  pub fn allow_other_versions(&mut self, allow: bool) -> &mut Self {
    self.allow_other_versions = allow;
    self
  }

  pub fn add_bundle(&mut self, bundle: MockBundle) -> &mut Self {
    {
      let mut collection = self.bundles.lock().unwrap();
      collection.add(bundle);
    }
    self
  }

  pub fn remove_bundle(
    &mut self,
    name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    let name = name.into();
    let version = version.into();
    {
      let mut collection = self.bundles.lock().unwrap();
      collection.remove(&name, &version);
    }
    {
      let mut cv = self.current_versions.lock().unwrap();
      cv.unset_if_current(&name, &version);
    }
    self
  }

  pub fn remove_bundle_all(&mut self, name: impl Into<String>) -> &mut Self {
    let name = name.into();
    {
      let mut collection = self.bundles.lock().unwrap();
      collection.remove_all(&name);
    }
    {
      let mut cv = self.current_versions.lock().unwrap();
      cv.unset(&name);
    }
    self
  }

  pub fn add_channel_bundle(
    &mut self,
    channel: impl Into<String>,
    bundle: MockBundle,
  ) -> &mut Self {
    let channel = channel.into();
    {
      let mut collection = self.channel_bundles.lock().unwrap();
      collection
        .entry(channel)
        .and_modify(|x| {
          x.add(bundle.clone());
        })
        .or_insert_with(move || {
          let mut collection = MockBundleCollection::new();
          collection.add(bundle);
          collection
        });
    }
    self
  }

  pub fn remove_channel_bundle(
    &mut self,
    channel: impl Into<String>,
    name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    let channel = channel.into();
    let name = name.into();
    let version = version.into();
    {
      let mut collection = self.channel_bundles.lock().unwrap();
      if let Some(x) = collection.get_mut(&channel) {
        x.remove(&name, &version);
      }
    }
    {
      let mut cv = self.channel_current_versions.lock().unwrap();
      if let Some(x) = cv.get_mut(&channel) {
        x.unset_if_current(&name, &version);
      }
    }
    self
  }

  pub fn remove_channel_bundle_all(
    &mut self,
    channel: impl Into<String>,
    name: impl Into<String>,
  ) -> &mut Self {
    let channel = channel.into();
    let name = name.into();
    {
      let mut collection = self.channel_bundles.lock().unwrap();
      if let Some(x) = collection.get_mut(&channel) {
        x.remove_all(&name);
      }
    }
    {
      let mut cv = self.channel_current_versions.lock().unwrap();
      if let Some(x) = cv.get_mut(&channel) {
        x.unset(&name);
      }
    }
    self
  }

  pub fn set_bundle_current_version(
    &mut self,
    name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    {
      let mut cv = self.current_versions.lock().unwrap();
      cv.set(name, version);
    }
    self
  }

  pub fn set_channel_bundle_current_version(
    &mut self,
    channel: impl Into<String>,
    name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    let channel = channel.into();
    let name = name.into();
    let version = version.into();
    {
      let mut cv = self.channel_current_versions.lock().unwrap();
      cv.entry(channel)
        .and_modify(|x| {
          x.set(&name, &version);
        })
        .or_insert_with(|| {
          let mut cv = CurrentVersions::new();
          cv.set(&name, &version);
          cv
        });
    }
    self
  }

  fn init(&mut self) {
    let bundles = Arc::clone(&self.bundles);
    let current_versions = Arc::clone(&self.current_versions);
    let channel_bundles = Arc::clone(&self.channel_bundles);
    let channel_current_versions = Arc::clone(&self.channel_current_versions);

    let get_bundle = Arc::new(
      move |bundle_name: String,
            version: Option<String>,
            channel: Option<String>|
            -> Option<MockBundle> {
        let version = version.or_else(|| match &channel {
          Some(c) => {
            let ccv = channel_current_versions.lock().unwrap();
            ccv
              .get(c)
              .and_then(|x| x.versions.get(&bundle_name))
              .cloned()
          }
          None => {
            let cv = current_versions.lock().unwrap();
            cv.versions.get(&bundle_name).cloned()
          }
        })?;

        let target: MockBundle = (bundle_name, version).into();
        let bundle = match &channel {
          Some(c) => {
            let cb = channel_bundles.lock().unwrap();
            cb.get(c).and_then(|x| x.bundles.get(&target)).cloned()
          }
          None => {
            let b = bundles.lock().unwrap();
            b.bundles.get(&target).cloned()
          }
        }?;
        Some(bundle)
      },
    );

    let list_bundles = self.server.mock(|when, then| {
      #[derive(Serialize)]
      struct ResponseItem {
        name: String,
        version: String,
      }

      let bundles = Arc::clone(&self.bundles);
      let channel_bundles = Arc::clone(&self.channel_bundles);

      when.method("GET").path("/bundles");
      then.respond_with(move |req| {
        let channel = get_channel(req);
        let resp = match channel {
          Some(c) => {
            let cb = channel_bundles.lock().unwrap();
            cb.get(&c)
              .map(|x| x.bundles.iter())
              .unwrap_or_default()
              .map(|x| ResponseItem {
                name: x.name().to_owned(),
                version: x.version().to_owned(),
              })
              .collect::<Vec<_>>()
          }
          None => {
            let b = bundles.lock().unwrap();
            b.bundles
              .iter()
              .map(|x| ResponseItem {
                name: x.name().to_owned(),
                version: x.version().to_owned(),
              })
              .collect::<Vec<_>>()
          }
        };
        HttpMockResponse::builder()
          .status(200)
          .header("content-type", "application/json")
          .body(serde_json::to_string(&resp).unwrap())
          .build()
      });
    });

    let get_current_info = self.server.mock(|when, then| {
      let gb = Arc::clone(&get_bundle);
      when.method("HEAD").path_matches(r"^/bundles/([^/]+)$");
      then.respond_with(move |req| {
        let channel = get_channel(req);
        let bundle_name = get_bundle_name(req);
        let bundle = if let Some(b) = gb(bundle_name, None, channel) {
          b
        } else {
          return HttpMockResponse::builder().status(404).build();
        };
        HttpMockResponse::builder()
          .status(204)
          .headers(bundle.remote_headers())
          .build()
      });
    });

    let download = self.server.mock(|when, then| {
      let gb = Arc::clone(&get_bundle);
      when.method("GET").path_matches(r"^/bundles/([^/]+)$");
      then.respond_with(move |req| {
        let channel = get_channel(req);
        let bundle_name = get_bundle_name(req);
        let bundle = if let Some(b) = gb(bundle_name, None, channel) {
          b
        } else {
          return HttpMockResponse::builder().status(404).build();
        };
        HttpMockResponse::builder()
          .status(200)
          .headers(bundle.remote_headers())
          .header("content-type", "application/webview-bundle")
          .body(bundle.bundle_data())
          .build()
      });
    });

    let download_version = self.server.mock(|when, then| {
      let gb = Arc::clone(&get_bundle);
      let allow_other_versions = self.allow_other_versions;

      when
        .method("GET")
        .path_matches(r"^/bundles/([^/]+)/([^/]+)$");
      then.respond_with(move |req| {
        if !allow_other_versions {
          return HttpMockResponse::builder().status(403).build();
        }
        let channel = get_channel(req);
        let bundle_name = get_bundle_name(req);
        let version = get_version(req);
        let bundle = if let Some(b) = gb(bundle_name, Some(version), channel) {
          b
        } else {
          return HttpMockResponse::builder().status(404).build();
        };
        HttpMockResponse::builder()
          .status(200)
          .headers(bundle.remote_headers())
          .header("content-type", "application/webview-bundle")
          .body(bundle.bundle_data())
          .build()
      });
    });

    self
      .mocks
      .insert(MockRemoteEndpoint::ListBundles, list_bundles.id());
    self
      .mocks
      .insert(MockRemoteEndpoint::GetCurrentInfo, get_current_info.id());
    self
      .mocks
      .insert(MockRemoteEndpoint::Download, download.id());
    self
      .mocks
      .insert(MockRemoteEndpoint::DownloadVersion, download_version.id());
  }
}

fn get_channel(req: &HttpMockRequest) -> Option<String> {
  req.query_params_map().get("channel").cloned()
}

fn get_bundle_name(req: &HttpMockRequest) -> String {
  // /bundles/:bundle_name -> Some(bundle_name)
  // /bundles/:bundle_name/:version -> Some(bundle_name)
  req
    .uri()
    .path()
    .split('/')
    .nth(2)
    .map(String::from)
    .unwrap()
}

fn get_version(req: &HttpMockRequest) -> String {
  // /bundles/:bundle_name -> None
  // /bundles/:bundle_name/:version -> Some(version)
  req
    .uri()
    .path()
    .split('/')
    .nth(3)
    .map(String::from)
    .unwrap()
}

pub struct MockSystem {
  source: MockSource,
  remote: MockRemote,
}

impl MockSystem {
  pub fn new() -> Self {
    Self {
      source: MockSource::new(),
      remote: MockRemote::new(),
    }
  }

  pub fn source(&self) -> &MockSource {
    &self.source
  }

  pub fn source_mut(&mut self) -> &mut MockSource {
    &mut self.source
  }

  pub fn remote(&self) -> &MockRemote {
    &self.remote
  }

  pub fn remote_mut(&mut self) -> &mut MockRemote {
    &mut self.remote
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::protocol::{BundleProtocol, Protocol};
  use crate::remote::ListRemoteBundleInfo;
  use crate::updater::Updater;
  use http::Request;

  #[tokio::test]
  async fn smoke() {
    let mut system = MockSystem::new();
    system
      .source_mut()
      .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
        "/index.html",
        BundleEntry::new(b"<h1>1.0.0</h1>", "text/html", None),
      ))
      .set_builtin_current_version("app", "1.0.0");
    system
      .remote_mut()
      .add_bundle(MockBundle::new("app", "1.1.0").with_entry(
        "/index.html",
        BundleEntry::new(b"<h1>1.1.0</h1>", "text/html", None),
      ))
      .set_bundle_current_version("app", "1.1.0");

    let source = Arc::new(system.source().get_source());
    let protocol = BundleProtocol::new(source.clone());
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    let html = str::from_utf8(resp.body()).unwrap();
    assert_eq!(html, "<h1>1.0.0</h1>");

    let remote = Arc::new(system.remote().get_remote());

    let updater = Updater::new(source.clone(), remote.clone(), None);
    let remotes = updater.list_remotes().await.unwrap();
    assert_eq!(remotes.len(), 1);
    assert_eq!(
      remotes,
      vec![ListRemoteBundleInfo {
        name: "app".to_string(),
        version: "1.1.0".to_string(),
      }]
    );
    let update_info = updater.get_update("app").await.unwrap();
    assert_eq!(update_info.name, "app");
    assert_eq!(update_info.version, "1.1.0");
    assert_eq!(update_info.local_version.unwrap(), "1.0.0");
    assert!(update_info.is_available);

    updater.download_update("app", None).await.unwrap();
    source.update_version("app", "1.1.0").await.unwrap();

    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    let html = str::from_utf8(resp.body()).unwrap();
    assert_eq!(html, "<h1>1.1.0</h1>");
  }
}
