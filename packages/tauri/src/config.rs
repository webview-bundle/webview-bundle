use std::collections::HashMap;
use std::path::PathBuf;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, Runtime};
use wvb::remote;

pub use wvb::remote::HttpConfig as Http;

type DynamicDirFn<R> = fn(app: &AppHandle<R>) -> Result<PathBuf, Box<dyn std::error::Error>>;

#[derive(Clone)]
pub(crate) enum Dir<R: Runtime> {
  Static(String),
  Dynamic(DynamicDirFn<R>),
}

impl<R: Runtime> Dir<R> {
  pub fn resolve(&self, app: &AppHandle<R>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    match self {
      Self::Static(dir) => {
        let parsed = app.path().parse(dir)?;
        Ok(parsed)
      }
      Self::Dynamic(f) => {
        let dir = f(app)?;
        Ok(dir)
      }
    }
  }
}

#[derive(Clone, Default)]
pub struct Source<R: Runtime> {
  pub(crate) builtin_dir: Option<Dir<R>>,
  pub(crate) remote_dir: Option<Dir<R>>,
}

impl<R: Runtime> Source<R> {
  pub fn new() -> Self {
    Self {
      builtin_dir: None,
      remote_dir: None,
    }
  }

  pub fn builtin_dir<T: Into<String>>(mut self, dir: T) -> Self {
    self.builtin_dir = Some(Dir::Static(dir.into()));
    self
  }

  pub fn builtin_dir_fn(mut self, dir: DynamicDirFn<R>) -> Self {
    self.builtin_dir = Some(Dir::Dynamic(dir));
    self
  }

  pub fn remote_dir<T: Into<String>>(mut self, dir: T) -> Self {
    self.remote_dir = Some(Dir::Static(dir.into()));
    self
  }

  pub fn remote_dir_fn(mut self, dir: DynamicDirFn<R>) -> Self {
    self.remote_dir = Some(Dir::Dynamic(dir));
    self
  }

  pub(crate) fn resolve_builtin_dir(&self, app: &AppHandle<R>) -> crate::Result<PathBuf> {
    let dir = match self.builtin_dir {
      Some(ref builtin_dir) => builtin_dir
        .resolve(app)
        .map_err(|e| crate::Error::FailToResolveDirectory(e.to_string()))?,
      None => app.path().resolve("bundles", BaseDirectory::Resource)?,
    };
    Ok(dir)
  }

  pub(crate) fn resolve_remote_dir(&self, app: &AppHandle<R>) -> crate::Result<PathBuf> {
    let dir = match self.remote_dir {
      Some(ref remote_dir) => remote_dir
        .resolve(app)
        .map_err(|e| crate::Error::FailToResolveDirectory(e.to_string()))?,
      None => {
        // Downloaded bundles must live in a writable, per-user location.
        app.path().resolve("bundles", BaseDirectory::AppLocalData)?
      }
    };
    Ok(dir)
  }
}

#[derive(Clone, Default)]
pub struct Remote {
  builder: remote::RemoteBuilder,
}

impl Remote {
  pub fn new(endpoint: impl Into<String>) -> Self {
    let builder = remote::Remote::builder().endpoint(endpoint);
    Self { builder }
  }

  pub fn http(mut self, http: Http) -> Self {
    self.builder = self.builder.http(http);
    self
  }

  pub fn on_download<F>(mut self, on_download: F) -> Self
  where
    F: Fn(u64, u64, String) + Send + Sync + 'static,
  {
    self.builder = self.builder.on_download(on_download);
    self
  }

  pub(crate) fn build(self) -> crate::Result<remote::Remote> {
    let remote = self.builder.build()?;
    Ok(remote)
  }
}

#[derive(Clone)]
pub struct BundleProtocolConfig {
  scheme: String,
}

impl BundleProtocolConfig {
  pub fn new<S: Into<String>>(scheme: S) -> Self {
    Self {
      scheme: scheme.into(),
    }
  }
}

#[derive(Clone)]
pub struct LocalProtocolConfig {
  scheme: String,
  pub(crate) hosts: HashMap<String, String>,
}

impl LocalProtocolConfig {
  pub fn new<S: Into<String>>(scheme: S) -> Self {
    Self {
      scheme: scheme.into(),
      hosts: HashMap::new(),
    }
  }

  pub fn new_with_hosts<T: Into<HashMap<String, String>>>(scheme: String, hosts: T) -> Self {
    Self {
      scheme,
      hosts: hosts.into(),
    }
  }

  pub fn host<T: Into<String>, U: Into<String>>(mut self, host: T, url: U) -> Self {
    self.hosts.insert(host.into(), url.into());
    self
  }

  pub fn hosts<T: Into<HashMap<String, String>>>(mut self, hosts: T) -> Self {
    self.hosts = hosts.into();
    self
  }
}

#[derive(Clone)]
pub enum Protocol {
  Bundle(BundleProtocolConfig),
  Local(LocalProtocolConfig),
}

impl Protocol {
  pub fn bundle<S: Into<String>>(scheme: S) -> BundleProtocolConfig {
    BundleProtocolConfig::new(scheme)
  }

  pub fn local<S: Into<String>>(scheme: S) -> LocalProtocolConfig {
    LocalProtocolConfig::new(scheme)
  }

  pub fn scheme(&self) -> &str {
    match self {
      Protocol::Bundle(x) => &x.scheme,
      Protocol::Local(x) => &x.scheme,
    }
  }
}

impl From<BundleProtocolConfig> for Protocol {
  fn from(value: BundleProtocolConfig) -> Self {
    Protocol::Bundle(value)
  }
}

impl From<LocalProtocolConfig> for Protocol {
  fn from(value: LocalProtocolConfig) -> Self {
    Protocol::Local(value)
  }
}

#[derive(Clone, Default)]
pub struct Config<R: Runtime> {
  pub(crate) source: Source<R>,
  pub(crate) protocols: Vec<Protocol>,
  pub(crate) remote: Option<Remote>,
}

impl<R: Runtime> Config<R> {
  pub fn new() -> Self {
    Self {
      source: Source::new(),
      protocols: vec![],
      remote: Default::default(),
    }
  }

  pub fn source(mut self, source: Source<R>) -> Self {
    self.source = source;
    self
  }

  pub fn protocol<P: Into<Protocol>>(mut self, protocol: P) -> Self {
    self.protocols.push(protocol.into());
    self
  }

  pub fn remote(mut self, remote: Remote) -> Self {
    self.remote = Some(remote);
    self
  }

  pub(crate) fn build_remote(&self) -> crate::Result<Option<remote::Remote>> {
    if let Some(ref remote_config) = self.remote {
      let remote = remote_config.clone().build()?;
      Ok(Some(remote))
    } else {
      Ok(None)
    }
  }
}
