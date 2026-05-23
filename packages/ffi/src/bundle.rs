use crate::http::HttpHeaders;
use crate::mime::MimeType;
use crate::version::Version;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use wvb::http::HeaderMap;
use wvb::{
  AsyncBundleReader, AsyncBundleWriter, AsyncReader, AsyncWriter, BundleBuilderOptions,
  BundleEntry, BundleReader, BundleWriter, HeaderWriterOptions, IndexWriterOptions, Reader, Writer,
};

#[derive(uniffi::Object)]
pub struct Header {
  pub(crate) inner: wvb::Header,
}

#[uniffi::export]
impl Header {
  pub fn version(&self) -> Version {
    Version::from(self.inner.version())
  }

  pub fn index_end_offset(&self) -> u64 {
    self.inner.index_end_offset()
  }

  pub fn index_size(&self) -> u32 {
    self.inner.index_size()
  }
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct IndexEntry {
  pub offset: u64,
  pub len: u64,
  pub is_empty: bool,
  pub content_type: String,
  pub content_length: u64,
  pub headers: HashMap<String, String>,
}

impl From<&wvb::IndexEntry> for IndexEntry {
  fn from(value: &wvb::IndexEntry) -> Self {
    IndexEntry {
      offset: value.offset(),
      len: value.len(),
      is_empty: value.is_empty(),
      content_type: value.content_type().to_string(),
      content_length: value.content_length(),
      headers: HttpHeaders::from(value.headers()).0,
    }
  }
}

#[derive(uniffi::Object)]
pub struct Index {
  bundle: Arc<wvb::Bundle>,
}

#[uniffi::export]
impl Index {
  pub fn entries(&self) -> HashMap<String, IndexEntry> {
    self
      .bundle
      .descriptor()
      .index()
      .entries()
      .iter()
      .map(|(k, v)| (k.to_string(), IndexEntry::from(v)))
      .collect()
  }

  pub fn get_entry(&self, path: String) -> Option<IndexEntry> {
    self
      .bundle
      .descriptor()
      .index()
      .get_entry(&path)
      .map(IndexEntry::from)
  }

  pub fn contains_path(&self, path: String) -> bool {
    self.bundle.descriptor().index().contains_path(&path)
  }
}

pub(crate) enum BundleDescriptorInner {
  Owned(wvb::BundleDescriptor),
  Bundle(Arc<wvb::Bundle>),
}

#[derive(uniffi::Object)]
pub struct BundleDescriptor {
  pub(crate) inner: BundleDescriptorInner,
}

impl BundleDescriptor {
  fn descriptor(&self) -> &wvb::BundleDescriptor {
    match &self.inner {
      BundleDescriptorInner::Owned(d) => d,
      BundleDescriptorInner::Bundle(b) => b.descriptor(),
    }
  }
}

#[uniffi::export]
impl BundleDescriptor {
  pub fn header(&self) -> Arc<Header> {
    Arc::new(Header {
      inner: self.descriptor().header().clone(),
    })
  }

  pub fn index(&self) -> Arc<Index> {
    match &self.inner {
      BundleDescriptorInner::Bundle(b) => Arc::new(Index { bundle: b.clone() }),
      BundleDescriptorInner::Owned(_) => {
        // For owned descriptors (from fetch_descriptor), build a stub bundle is not possible.
        // Return an index that reads directly from the owned descriptor.
        // We clone the entries into a temporary bundle-less index wrapper by creating
        // a detached index.
        panic!(
          "BundleDescriptor from fetch_descriptor does not support index(). Use fetch() instead."
        );
      }
    }
  }

  pub fn index_entries(&self) -> HashMap<String, IndexEntry> {
    self
      .descriptor()
      .index()
      .entries()
      .iter()
      .map(|(k, v)| (k.to_string(), IndexEntry::from(v)))
      .collect()
  }

  pub fn get_index_entry(&self, path: String) -> Option<IndexEntry> {
    self
      .descriptor()
      .index()
      .get_entry(&path)
      .map(IndexEntry::from)
  }

  pub fn contains_path(&self, path: String) -> bool {
    self.descriptor().index().contains_path(&path)
  }
}

#[derive(uniffi::Object, Debug)]
pub struct Bundle {
  pub(crate) inner: Arc<wvb::Bundle>,
}

#[uniffi::export]
impl Bundle {
  pub fn descriptor(&self) -> Arc<BundleDescriptor> {
    Arc::new(BundleDescriptor {
      inner: BundleDescriptorInner::Bundle(self.inner.clone()),
    })
  }

  pub fn get_data(&self, path: String) -> Result<Option<Vec<u8>>, crate::Error> {
    Ok(self.inner.get_data(&path)?.map(|x| x.to_vec()))
  }

  pub fn get_data_checksum(&self, path: String) -> Result<Option<u32>, crate::Error> {
    Ok(self.inner.get_data_checksum(&path)?)
  }
}

#[uniffi::export]
pub fn read_bundle_from_bytes(data: Vec<u8>) -> Result<Arc<Bundle>, crate::Error> {
  let cursor = Cursor::new(data);
  let bundle = Reader::<wvb::Bundle>::read(&mut BundleReader::new(cursor))?;
  Ok(Arc::new(Bundle {
    inner: Arc::new(bundle),
  }))
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn read_bundle(filepath: String) -> Result<Arc<Bundle>, crate::Error> {
  let mut file = tokio::fs::File::open(&filepath)
    .await
    .map_err(wvb::Error::from)?;
  let bundle = AsyncReader::<wvb::Bundle>::read(&mut AsyncBundleReader::new(&mut file)).await?;
  Ok(Arc::new(Bundle {
    inner: Arc::new(bundle),
  }))
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn write_bundle(bundle: Arc<Bundle>, filepath: String) -> Result<u64, crate::Error> {
  let mut file = tokio::fs::File::create(&filepath)
    .await
    .map_err(wvb::Error::from)?;
  let size =
    AsyncWriter::<wvb::Bundle>::write(&mut AsyncBundleWriter::new(&mut file), &bundle.inner)
      .await?;
  Ok(size as u64)
}

#[uniffi::export]
pub fn write_bundle_to_bytes(bundle: Arc<Bundle>) -> Result<Vec<u8>, crate::Error> {
  let mut buf = vec![];
  Writer::<wvb::Bundle>::write(&mut BundleWriter::new(&mut buf), &bundle.inner)?;
  Ok(buf)
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct BuildOptions {
  pub header: Option<BuildHeaderOptions>,
  pub index: Option<BuildIndexOptions>,
  pub data_checksum_seed: Option<u32>,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct BuildHeaderOptions {
  pub checksum_seed: Option<u32>,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct BuildIndexOptions {
  pub checksum_seed: Option<u32>,
}

impl From<BuildHeaderOptions> for HeaderWriterOptions {
  fn from(value: BuildHeaderOptions) -> Self {
    let mut options = HeaderWriterOptions::new();
    if let Some(seed) = value.checksum_seed {
      options.checksum_seed(seed);
    }
    options
  }
}

impl From<BuildIndexOptions> for IndexWriterOptions {
  fn from(value: BuildIndexOptions) -> Self {
    let mut options = IndexWriterOptions::new();
    if let Some(seed) = value.checksum_seed {
      options.checksum_seed(seed);
    }
    options
  }
}

impl From<BuildOptions> for BundleBuilderOptions {
  fn from(value: BuildOptions) -> Self {
    let mut options = BundleBuilderOptions::new();
    if let Some(header) = value.header {
      options.header(header.into());
    }
    if let Some(index) = value.index {
      options.index(index.into());
    }
    if let Some(seed) = value.data_checksum_seed {
      options.data_checksum_seed(seed);
    }
    options
  }
}

#[derive(uniffi::Object)]
pub struct BundleBuilder {
  inner: Mutex<wvb::BundleBuilder>,
  version: Version,
}

#[uniffi::export]
impl BundleBuilder {
  #[uniffi::constructor]
  pub fn new(version: Option<Version>) -> Arc<BundleBuilder> {
    Arc::new(BundleBuilder {
      version: version.unwrap_or(Version::V1),
      inner: Mutex::new(wvb::BundleBuilder::new()),
    })
  }

  pub fn version(&self) -> Version {
    self.version.clone()
  }

  pub fn entry_paths(&self) -> Vec<String> {
    self
      .inner
      .lock()
      .unwrap()
      .entries()
      .keys()
      .map(|s| s.to_string())
      .collect()
  }

  pub fn insert_entry(
    &self,
    path: String,
    data: Vec<u8>,
    content_type: Option<String>,
    headers: Option<HashMap<String, String>>,
  ) -> Result<bool, crate::Error> {
    let headers = if let Some(h) = headers {
      Some(HeaderMap::try_from(HttpHeaders::from(h))?)
    } else {
      None
    };
    let content_type = content_type
      .unwrap_or_else(|| MimeType::parse_with_fallback(&data, &path, MimeType::OctetStream));
    let entry = BundleEntry::new(&data, content_type, headers);
    let replaced = self
      .inner
      .lock()
      .unwrap()
      .insert_entry(path, entry)
      .is_some();
    Ok(replaced)
  }

  pub fn remove_entry(&self, path: String) -> bool {
    self.inner.lock().unwrap().remove_entry(&path).is_some()
  }

  pub fn contains_entry(&self, path: String) -> bool {
    self.inner.lock().unwrap().contains_path(&path)
  }

  pub fn build(&self, options: Option<BuildOptions>) -> Result<Arc<Bundle>, crate::Error> {
    let mut inner = self.inner.lock().unwrap();
    if let Some(opts) = options {
      inner.set_options(opts.into());
    }
    let bundle = inner.build()?;
    Ok(Arc::new(Bundle {
      inner: Arc::new(bundle),
    }))
  }
}
