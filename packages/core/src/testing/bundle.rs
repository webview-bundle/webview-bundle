use crate::integrity::{Integrity, IntegrityAlgorithm};
use crate::source::ManifestVersionData;
use crate::{
  Bundle, BundleBuilderOptions, BundleDescriptor, BundleEntry, BundleReader, BundleWriter, Reader,
  Writer,
};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct TestingBundle {
  name: String,
  version: String,
  entries: HashMap<String, BundleEntry>,
  options: Option<BundleBuilderOptions>,
  metadata: Option<HashMap<String, String>>,
}

impl TestingBundle {
  pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      entries: Default::default(),
      options: None,
      metadata: None,
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

  pub fn make_version_data(
    &self,
    integrity_alg: Option<IntegrityAlgorithm>,
  ) -> anyhow::Result<ManifestVersionData> {
    let integrity = match integrity_alg {
      Some(alg) => Some(self.make_integrity(alg)?.serialize()),
      None => None,
    };
    Ok(ManifestVersionData {
      integrity,
      metadata: self.metadata.clone(),
    })
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

  pub fn set_metadata(&mut self, metadata: HashMap<String, String>) -> &mut Self {
    self.metadata = Some(metadata);
    self
  }

  /// The offset, within the bytes of the `.wvb` file, of the compressed data of `path`.
  ///
  /// An entry is stored as `[compressed bytes][4-byte checksum]`, so flipping a byte at this
  /// offset corrupts the payload while leaving its checksum intact — which is what a
  /// checksum-verifying read is supposed to catch. Flipping a byte of the payload's leading
  /// length prefix instead surfaces as a decompression error, so tests that mean to exercise
  /// the checksum should target this offset.
  pub fn entry_data_offset(&self, path: &str) -> anyhow::Result<usize> {
    let (descriptor, entry_offset, _) = self.entry_position(path)?;
    Ok((descriptor.header().index_end_offset() + entry_offset) as usize)
  }

  /// The offset, within the bytes of the `.wvb` file, of the 4-byte checksum of `path`.
  pub fn entry_checksum_offset(&self, path: &str) -> anyhow::Result<usize> {
    let (descriptor, entry_offset, entry_len) = self.entry_position(path)?;
    Ok((descriptor.header().index_end_offset() + entry_offset + entry_len) as usize)
  }

  fn entry_position(&self, path: &str) -> anyhow::Result<(BundleDescriptor, u64, u64)> {
    let data = self.make_bundle_data()?;
    let descriptor: BundleDescriptor = BundleReader::new(Cursor::new(&data)).read()?;
    let entry = descriptor
      .index()
      .get_entry(path)
      .ok_or_else(|| anyhow::anyhow!("no entry at {path:?}"))?;
    let (offset, len) = (entry.offset(), entry.len());
    Ok((descriptor, offset, len))
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
