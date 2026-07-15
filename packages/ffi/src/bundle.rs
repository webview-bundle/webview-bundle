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

/// Fixed-size bundle file header containing format metadata.
#[derive(uniffi::Object)]
pub struct Header {
  pub(crate) inner: wvb::Header,
}

#[uniffi::export]
impl Header {
  /// Bundle format version encoded in the header.
  pub fn version(&self) -> Version {
    Version::from(self.inner.version())
  }

  /// Byte offset at which the index section ends (and data section begins).
  pub fn index_end_offset(&self) -> u64 {
    self.inner.index_end_offset()
  }

  /// Byte length of the serialized index section.
  pub fn index_size(&self) -> u32 {
    self.inner.index_size()
  }
}

/// Metadata for a single entry stored in the bundle index.
///
/// `offset` and `len` refer to the entry's position in the data section.
/// `content_length` is the logical (uncompressed) size exposed to HTTP clients,
/// which may differ from `len` if the data is stored compressed.
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

/// View into the index section of a bundle. Backed by the parent [`Bundle`] via
/// an `Arc` so that individual entries can be read without copying the whole bundle.
#[derive(uniffi::Object)]
pub struct Index {
  bundle: Arc<wvb::Bundle>,
}

#[uniffi::export]
impl Index {
  /// Returns all index entries keyed by their path (e.g. `"/index.html"`).
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

/// Internal storage for [`BundleDescriptor`].
///
/// `Owned` holds a standalone descriptor (returned by `fetch_descriptor`); it
/// has no data section so `index()` is not supported on this variant.
/// `Arc` shares a cached descriptor (returned via `LoadedDescriptor::descriptor`);
/// like `Owned` it carries only metadata, so `index()` is not supported either.
/// `Bundle` shares the full bundle via `Arc` so both descriptor and data can be
/// accessed through the same object.
pub(crate) enum BundleDescriptorInner {
  Owned(wvb::BundleDescriptor),
  Arc(Arc<wvb::BundleDescriptor>),
  Bundle(Arc<wvb::Bundle>),
}

/// Header + index metadata for a bundle, without necessarily loading the data section.
#[derive(uniffi::Object)]
pub struct BundleDescriptor {
  pub(crate) inner: BundleDescriptorInner,
}

impl BundleDescriptor {
  fn descriptor(&self) -> &wvb::BundleDescriptor {
    match &self.inner {
      BundleDescriptorInner::Owned(d) => d,
      BundleDescriptorInner::Arc(d) => d,
      BundleDescriptorInner::Bundle(b) => b.descriptor(),
    }
  }
}

#[uniffi::export]
impl BundleDescriptor {
  /// Bundle file header.
  pub fn header(&self) -> Arc<Header> {
    Arc::new(Header {
      inner: *self.descriptor().header(),
    })
  }

  /// Returns an [`Index`] view backed by the full bundle.
  ///
  /// # Panics
  /// Panics when called on a metadata-only descriptor (obtained via
  /// `BundleSource::fetch_descriptor` or `LoadedDescriptor::descriptor`), because
  /// those variants have no data section. Use `BundleSource::fetch_bundle` instead
  /// when data access is required.
  pub fn index(&self) -> Arc<Index> {
    match &self.inner {
      BundleDescriptorInner::Bundle(b) => Arc::new(Index { bundle: b.clone() }),
      BundleDescriptorInner::Owned(_) | BundleDescriptorInner::Arc(_) => {
        panic!(
          "BundleDescriptor without a data section does not support index(). Use fetch_bundle() instead."
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

/// A fully loaded bundle, giving access to both descriptor metadata and raw entry data.
#[derive(uniffi::Object, Debug)]
pub struct Bundle {
  pub(crate) inner: Arc<wvb::Bundle>,
}

#[uniffi::export]
impl Bundle {
  /// Header and index metadata for this bundle.
  pub fn descriptor(&self) -> Arc<BundleDescriptor> {
    Arc::new(BundleDescriptor {
      inner: BundleDescriptorInner::Bundle(self.inner.clone()),
    })
  }

  /// Returns the raw bytes for the entry at `path`, or `None` if the path does not exist.
  pub fn get_data(&self, path: String) -> Result<Option<Vec<u8>>, crate::Error> {
    Ok(self.inner.get_data(&path)?.map(|x| x.to_vec()))
  }

  /// Returns the CRC-32 checksum of the data at `path`, or `None` if the path does not exist.
  pub fn get_data_checksum(&self, path: String) -> Result<Option<u32>, crate::Error> {
    Ok(self.inner.get_data_checksum(&path)?)
  }
}

/// Deserializes a bundle from an in-memory byte slice.
/// Prefer [`read_bundle`] for large files to avoid loading everything into memory.
#[uniffi::export]
pub fn read_bundle_from_bytes(data: Vec<u8>) -> Result<Arc<Bundle>, crate::Error> {
  let cursor = Cursor::new(data);
  let bundle = Reader::<wvb::Bundle>::read(&mut BundleReader::new(cursor))?;
  Ok(Arc::new(Bundle {
    inner: Arc::new(bundle),
  }))
}

/// Reads a bundle from a file path using async I/O.
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

/// Writes a bundle to a file path using async I/O. Returns the number of bytes written.
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

/// Serializes a bundle into an in-memory byte vector.
#[uniffi::export]
pub fn write_bundle_to_bytes(bundle: Arc<Bundle>) -> Result<Vec<u8>, crate::Error> {
  let mut buf = vec![];
  Writer::<wvb::Bundle>::write(&mut BundleWriter::new(&mut buf), &bundle.inner)?;
  Ok(buf)
}

/// Top-level options passed to [`BundleBuilder::build`].
/// All fields are optional; omitting them applies the library defaults.
#[derive(uniffi::Record, Clone, Debug)]
pub struct BuildOptions {
  pub header: Option<BuildHeaderOptions>,
  pub index: Option<BuildIndexOptions>,
  /// Seed for the CRC-32 checksum written into each data entry.
  pub data_checksum_seed: Option<u32>,
}

/// Checksum options for the bundle header section.
#[derive(uniffi::Record, Clone, Debug)]
pub struct BuildHeaderOptions {
  pub checksum_seed: Option<u32>,
}

/// Checksum options for the bundle index section.
#[derive(uniffi::Record, Clone, Debug)]
pub struct BuildIndexOptions {
  pub checksum_seed: Option<u32>,
}

impl From<BuildHeaderOptions> for HeaderWriterOptions {
  fn from(value: BuildHeaderOptions) -> Self {
    let mut options = HeaderWriterOptions::default();
    if let Some(seed) = value.checksum_seed {
      options.checksum_seed(seed);
    }
    options
  }
}

impl From<BuildIndexOptions> for IndexWriterOptions {
  fn from(value: BuildIndexOptions) -> Self {
    let mut options = IndexWriterOptions::default();
    if let Some(seed) = value.checksum_seed {
      options.checksum_seed(seed);
    }
    options
  }
}

impl From<BuildOptions> for BundleBuilderOptions {
  fn from(value: BuildOptions) -> Self {
    let mut options = BundleBuilderOptions::default();
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

/// Incrementally assembles a bundle from individual entries before finalizing it with [`build`](BundleBuilder::build).
///
/// `BundleBuilder` is internally guarded by a `Mutex` so it is safe to share across
/// threads, though in practice it is typically used from a single thread.
#[derive(uniffi::Object)]
pub struct BundleBuilder {
  inner: Mutex<wvb::BundleBuilder>,
  version: Version,
}

#[uniffi::export]
impl BundleBuilder {
  /// Creates a new builder. Defaults to [`Version::V1`] when `version` is `None`.
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

  /// Inserts an entry at `path`. Returns `true` if a previous entry was replaced.
  ///
  /// `content_type` is inferred from the data bytes and file extension when `None`.
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

  /// Removes the entry at `path`. Returns `true` if an entry existed.
  pub fn remove_entry(&self, path: String) -> bool {
    self.inner.lock().unwrap().remove_entry(&path).is_some()
  }

  pub fn contains_entry(&self, path: String) -> bool {
    self.inner.lock().unwrap().contains_path(&path)
  }

  /// Finalizes the builder and produces a [`Bundle`].
  /// The builder remains usable after calling `build`.
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
