use crate::integrity::{Integrity, IntegrityAlgorithm};
use crate::{Bundle, BundleBuilderOptions, BundleEntry, BundleWriter, Writer};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct TestingBundle {
  name: String,
  version: String,
  entries: HashMap<String, BundleEntry>,
  options: Option<BundleBuilderOptions>,
}

impl TestingBundle {
  pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      entries: Default::default(),
      options: None,
    }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn version(&self) -> &str {
    &self.version
  }

  pub fn set_options(&mut self, options: BundleBuilderOptions) -> &mut Self {
    self.options = Some(options);
    self
  }

  pub fn make_bundle(&self) -> anyhow::Result<Bundle> {
    let mut builder = Bundle::builder();
    if let Some(options) = self.options {
      builder.set_options(options);
    }
    for (path, entry) in self.entries.iter() {
      builder.insert_entry(path, entry.clone());
    }
    let bundle = builder.build()?;
    Ok(bundle)
  }

  pub fn make_bundle_data(&self) -> anyhow::Result<Vec<u8>> {
    let mut data = vec![];
    let mut writer = BundleWriter::new(Cursor::new(&mut data));

    let bundle = self.make_bundle()?;
    writer.write(&bundle)?;

    Ok(data)
  }

  pub fn make_integrity(&self, alg: IntegrityAlgorithm) -> anyhow::Result<Integrity> {
    let data = self.make_bundle_data()?;
    Ok(Integrity::compute(alg, &data))
  }

  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    self
  }

  pub fn set_version(&mut self, version: impl Into<String>) -> &mut Self {
    self.version = version.into();
    self
  }

  pub fn add_entry(&mut self, path: impl Into<String>, entry: BundleEntry) -> &mut Self {
    self.entries.insert(path.into(), entry);
    self
  }
}

impl PartialEq for TestingBundle {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name && self.version == other.version
  }
}

impl Eq for TestingBundle {}

impl Hash for TestingBundle {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.name.hash(state);
    self.version.hash(state);
  }
}

impl<N, V> From<(N, V)> for TestingBundle
where
  N: Into<String>,
  V: Into<String>,
{
  fn from(value: (N, V)) -> Self {
    Self::new(value.0, value.1)
  }
}
