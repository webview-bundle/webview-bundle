use crate::checksum::{CHECKSUM_LEN, make_checksum};
use crate::header::HeaderWriterOptions;
use crate::index::{Index, IndexEntry, IndexWriterOptions};
use crate::version::Version;
use crate::{Bundle, BundleDescriptor, Header, IndexWriter, Writer};
use http::HeaderMap;
use lz4_flex::compress_prepend_size;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub struct BundleEntry {
  compressed: Vec<u8>,
  content_type: String,
  content_length: u64,
  pub headers: Option<HeaderMap>,
}

impl BundleEntry {
  pub fn new(data: &[u8], content_type: impl Into<String>, headers: Option<HeaderMap>) -> Self {
    let compressed = compress_prepend_size(data);
    Self {
      compressed,
      content_type: content_type.into(),
      content_length: data.len() as u64,
      headers,
    }
  }

  pub fn data(&self) -> &[u8] {
    &self.compressed
  }

  pub fn is_empty(&self) -> bool {
    self.compressed.is_empty()
  }

  pub fn content_type(&self) -> &str {
    &self.content_type
  }

  pub fn content_length(&self) -> u64 {
    self.content_length
  }

  pub fn len(&self) -> usize {
    self.compressed.len()
  }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct BundleBuilderOptions {
  pub(crate) header: HeaderWriterOptions,
  pub(crate) index: IndexWriterOptions,
  pub(crate) data_checksum_seed: u32,
}

impl BundleBuilderOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn header(&mut self, options: HeaderWriterOptions) -> &mut Self {
    self.header = options;
    self
  }

  pub fn index(&mut self, options: IndexWriterOptions) -> &mut Self {
    self.index = options;
    self
  }

  pub fn data_checksum_seed(&mut self, seed: u32) -> &mut Self {
    self.data_checksum_seed = seed;
    self
  }
}

#[derive(Debug, Default)]
pub struct BundleBuilder {
  entries: HashMap<String, BundleEntry>,
  version: Version,
  options: BundleBuilderOptions,
}

impl BundleBuilder {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn new_with_capacity(capacity: usize) -> Self {
    Self {
      entries: HashMap::with_capacity(capacity),
      ..Self::default()
    }
  }

  pub fn new_with_options(options: BundleBuilderOptions) -> Self {
    Self {
      options,
      ..Self::default()
    }
  }

  pub fn version(&self) -> Version {
    self.version
  }

  pub fn set_version(&mut self, version: Version) -> &mut Self {
    self.version = version;
    self
  }

  pub fn options(&self) -> &BundleBuilderOptions {
    &self.options
  }

  pub fn set_options(&mut self, options: BundleBuilderOptions) -> &mut Self {
    self.options = options;
    self
  }

  pub fn entries(&self) -> &HashMap<String, BundleEntry> {
    &self.entries
  }

  pub fn insert_entry<S: Into<String>, E: Into<BundleEntry>>(
    &mut self,
    path: S,
    entry: E,
  ) -> Option<BundleEntry> {
    let p: String = path.into();
    let e: BundleEntry = entry.into();
    self.entries.insert(p, e)
  }

  pub fn get_entry(&self, path: &str) -> Option<&BundleEntry> {
    self.entries.get(path)
  }

  pub fn get_entry_mut(&mut self, path: &str) -> Option<&mut BundleEntry> {
    self.entries.get_mut(path)
  }

  pub fn remove_entry(&mut self, path: &str) -> Option<BundleEntry> {
    self.entries.remove(path)
  }

  pub fn contains_path(&self, path: &str) -> bool {
    self.entries.contains_key(path)
  }

  pub fn build(&self) -> crate::Result<Bundle> {
    let index = self.build_index();
    let header = self.build_header(&index)?;
    let manifest = BundleDescriptor { header, index };
    let data = self.build_data();
    Ok(Bundle {
      descriptor: manifest,
      data,
    })
  }

  pub(crate) fn build_header(&self, index: &Index) -> crate::Result<Header> {
    let index_bytes_size =
      IndexWriter::new_with_options(&mut vec![], self.options().index).write(index)?;
    let index_size = (index_bytes_size - CHECKSUM_LEN) as u32;
    let header = Header::new(self.version(), index_size);
    Ok(header)
  }

  pub(crate) fn build_index(&self) -> Index {
    let mut index = Index::new_with_capacity(self.entries().len());
    let mut offset = 0;
    for (path, entry) in self.entries() {
      let len = entry.len() as u64;
      let mut index_entry =
        IndexEntry::new(offset, len, entry.content_type(), entry.content_length);
      if let Some(headers) = entry.headers.as_ref() {
        index_entry.headers.clone_from(headers);
      }
      index.insert_entry(path, index_entry);
      offset += len;
      offset += CHECKSUM_LEN as u64;
    }
    index
  }

  pub(crate) fn build_data(&self) -> Vec<u8> {
    let mut data = vec![];
    for entry in self.entries().values() {
      let checksum = make_checksum(self.options.data_checksum_seed, entry.data());
      data.extend_from_slice(entry.data());
      data.extend_from_slice(&checksum.to_be_bytes());
    }
    data
  }
}
