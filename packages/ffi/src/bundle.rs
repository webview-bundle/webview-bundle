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

/// View into the index section of a bundle.
///
/// Holds the index metadata itself, so it is available on a metadata-only descriptor
/// (from `BundleSource::fetch_descriptor`) as well as on a fully loaded [`Bundle`].
#[derive(uniffi::Object)]
pub struct Index {
  inner: wvb::Index,
}

#[uniffi::export]
impl Index {
  /// Returns all index entries keyed by their path (e.g. `"/index.html"`).
  pub fn entries(&self) -> HashMap<String, IndexEntry> {
    self
      .inner
      .entries()
      .iter()
      .map(|(k, v)| (k.to_string(), IndexEntry::from(v)))
      .collect()
  }

  pub fn get_entry(&self, path: String) -> Option<IndexEntry> {
    self.inner.get_entry(&path).map(IndexEntry::from)
  }

  pub fn contains_path(&self, path: String) -> bool {
    self.inner.contains_path(&path)
  }
}

/// Internal storage for [`BundleDescriptor`].
///
/// `Owned` holds a standalone descriptor (returned by `fetch_descriptor`) and `Arc` a
/// cached one (returned via `LoadedDescriptor::descriptor`); both carry only header and
/// index metadata, so reading an entry's data means reopening the bundle file by path.
/// `Bundle` shares the full bundle via `Arc`, so its data section is already in memory.
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

  /// Returns the bundle's [`Index`].
  pub fn index(&self) -> Arc<Index> {
    Arc::new(Index {
      inner: self.descriptor().index().clone(),
    })
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

  /// Reads the entry at `path` out of the bundle file at `filepath`.
  ///
  /// A descriptor carries only header and index metadata, so the bundle file is reopened
  /// to read the entry's bytes on demand. Returns `None` when `path` is not in the bundle.
  pub fn get_data(&self, filepath: String, path: String) -> Result<Option<Vec<u8>>, crate::Error> {
    let file = open_file(&filepath)?;
    let data = self.descriptor().get_data(file, &path)?;
    Ok(data)
  }

  /// Reads the stored xxHash-32 checksum of the entry at `path` out of the bundle file at
  /// `filepath`. Returns `None` when `path` is not in the bundle.
  pub fn get_data_checksum(
    &self,
    filepath: String,
    path: String,
  ) -> Result<Option<u32>, crate::Error> {
    let file = open_file(&filepath)?;
    let checksum = self.descriptor().get_data_checksum(file, &path)?;
    Ok(checksum)
  }
}

#[uniffi::export(async_runtime = "tokio")]
impl BundleDescriptor {
  /// Asynchronously reads the entry at `path` out of the bundle file at `filepath`.
  ///
  /// Returns `None` when `path` is not in the bundle.
  pub async fn async_get_data(
    &self,
    filepath: String,
    path: String,
  ) -> Result<Option<Vec<u8>>, crate::Error> {
    let file = async_open_file(&filepath).await?;
    let data = self.descriptor().async_get_data(file, &path).await?;
    Ok(data)
  }

  /// Asynchronously reads the stored xxHash-32 checksum of the entry at `path` out of the
  /// bundle file at `filepath`. Returns `None` when `path` is not in the bundle.
  pub async fn async_get_data_checksum(
    &self,
    filepath: String,
    path: String,
  ) -> Result<Option<u32>, crate::Error> {
    let file = async_open_file(&filepath).await?;
    let checksum = self
      .descriptor()
      .async_get_data_checksum(file, &path)
      .await?;
    Ok(checksum)
  }
}

fn open_file(filepath: &str) -> Result<std::fs::File, crate::Error> {
  std::fs::File::open(std::path::Path::new(filepath))
    .map_err(|e| crate::Error::from(wvb::Error::Io(e)))
}

async fn async_open_file(filepath: &str) -> Result<tokio::fs::File, crate::Error> {
  tokio::fs::File::open(std::path::Path::new(filepath))
    .await
    .map_err(|e| crate::Error::from(wvb::Error::Io(e)))
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
  pub data_checksum: Option<ChecksumWriteOptions>,
}

/// How each entry's xxHash-32 data checksum is verified when its data is read.
#[derive(uniffi::Record, Clone, Debug)]
pub struct ChecksumReadOptions {
  /// Verify each entry's data checksum when its data is read (default: `true`).
  #[uniffi(default = None)]
  pub verify: Option<bool>,
  /// The seed the data checksums were built with (default: `0`).
  #[uniffi(default = None)]
  pub seed: Option<u32>,
}

impl From<ChecksumReadOptions> for wvb::ChecksumReadOptions {
  fn from(value: ChecksumReadOptions) -> Self {
    let mut options = wvb::ChecksumReadOptions::default();
    if let Some(verify) = value.verify {
      options = options.verify(verify);
    }
    if let Some(seed) = value.seed {
      options = options.seed(seed);
    }
    options
  }
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct ChecksumWriteOptions {
  /// The seed the data checksums were built with (default: `0`).
  #[uniffi(default = None)]
  pub seed: Option<u32>,
}

impl From<ChecksumWriteOptions> for wvb::ChecksumWriteOptions {
  fn from(value: ChecksumWriteOptions) -> Self {
    let mut options = wvb::ChecksumWriteOptions::default();
    if let Some(seed) = value.seed {
      options = options.seed(seed);
    }
    options
  }
}

/// How each entry's data checksum is verified when its data is read.
#[derive(uniffi::Record, Clone, Debug)]
pub struct DataReadOptions {
  /// How each entry's data checksum is verified.
  #[uniffi(default = None)]
  pub checksum: Option<ChecksumReadOptions>,
}

impl From<DataReadOptions> for wvb::DataReadOptions {
  fn from(value: DataReadOptions) -> Self {
    let mut options = wvb::DataReadOptions::default();
    if let Some(checksum) = value.checksum {
      options = options.checksum(checksum.into());
    }
    options
  }
}

/// How a bundle's header checksum is verified when its header is read.
#[derive(uniffi::Record, Clone, Debug)]
pub struct HeaderReadOptions {
  /// How the header checksum is verified.
  #[uniffi(default = None)]
  pub checksum: Option<ChecksumReadOptions>,
}

impl From<HeaderReadOptions> for wvb::HeaderReadOptions {
  fn from(value: HeaderReadOptions) -> Self {
    let mut options = wvb::HeaderReadOptions::default();
    if let Some(checksum) = value.checksum {
      options = options.checksum(checksum.into());
    }
    options
  }
}
/// How a bundle's index checksum is verified when its index is read.
#[derive(uniffi::Record, Clone, Debug)]
pub struct IndexReadOptions {
  /// How the index checksum is verified.
  #[uniffi(default = None)]
  pub checksum: Option<ChecksumReadOptions>,
}

impl From<IndexReadOptions> for wvb::IndexReadOptions {
  fn from(value: IndexReadOptions) -> Self {
    let mut options = wvb::IndexReadOptions::default();
    if let Some(checksum) = value.checksum {
      options = options.checksum(checksum.into());
    }
    options
  }
}

/// Options for the bundle header section.
#[derive(uniffi::Record, Clone, Debug)]
pub struct BuildHeaderOptions {
  #[uniffi(default = None)]
  pub checksum: Option<ChecksumWriteOptions>,
}

/// Pptions for the bundle index section.
#[derive(uniffi::Record, Clone, Debug)]
pub struct BuildIndexOptions {
  #[uniffi(default = None)]
  pub checksum: Option<ChecksumWriteOptions>,
}

impl From<BuildHeaderOptions> for HeaderWriterOptions {
  fn from(value: BuildHeaderOptions) -> Self {
    let mut options = HeaderWriterOptions::default();
    if let Some(checksum) = value.checksum {
      options = options.checksum(checksum.into());
    }
    options
  }
}

impl From<BuildIndexOptions> for IndexWriterOptions {
  fn from(value: BuildIndexOptions) -> Self {
    let mut options = IndexWriterOptions::default();
    if let Some(checksum) = value.checksum {
      options = options.checksum(checksum.into());
    }
    options
  }
}

impl From<BuildOptions> for BundleBuilderOptions {
  fn from(value: BuildOptions) -> Self {
    let mut options = BundleBuilderOptions::default();
    if let Some(header) = value.header {
      options = options.header(header.into());
    }
    if let Some(index) = value.index {
      options = options.index(index.into());
    }
    if let Some(checksum) = value.data_checksum {
      options = options.data_checksum(checksum.into());
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

#[cfg(test)]
mod tests {
  use super::*;

  const BODY: &[u8] = b"<h1>hello</h1>";

  fn bundle() -> wvb::Bundle {
    let mut builder = wvb::BundleBuilder::new();
    builder.insert_entry("/index.html", BundleEntry::new(BODY, "text/html", None));
    builder.build().unwrap()
  }

  /// Writes `bundle` to a unique temp path and returns it. The caller removes it.
  fn write_to_temp(bundle: &wvb::Bundle, tag: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("wvb-ffi-{}-{tag}.wvb", std::process::id()));
    let mut buf = vec![];
    Writer::<wvb::Bundle>::write(&mut BundleWriter::new(&mut buf), bundle).unwrap();
    std::fs::write(&path, &buf).unwrap();
    path
  }

  /// A metadata-only descriptor reopens the file to read an entry's bytes on demand.
  #[test]
  fn descriptor_reads_entry_data_from_the_file() {
    let bundle = bundle();
    let path = write_to_temp(&bundle, "get-data");
    let descriptor = BundleDescriptor {
      inner: BundleDescriptorInner::Owned(bundle.descriptor().clone()),
    };
    let filepath = path.to_str().unwrap().to_string();

    let data = descriptor
      .get_data(filepath.clone(), "/index.html".to_string())
      .unwrap();
    assert_eq!(data.as_deref(), Some(BODY));

    // A path that is not in the bundle reads back as None rather than erroring.
    let missing = descriptor
      .get_data(filepath.clone(), "/nope.html".to_string())
      .unwrap();
    assert!(missing.is_none());

    // The checksum agrees with the one the fully loaded bundle reports.
    let checksum = descriptor
      .get_data_checksum(filepath, "/index.html".to_string())
      .unwrap();
    assert_eq!(checksum, bundle.get_data_checksum("/index.html").unwrap());

    std::fs::remove_file(&path).unwrap();
  }

  #[tokio::test]
  async fn descriptor_reads_entry_data_asynchronously() {
    let bundle = bundle();
    let path = write_to_temp(&bundle, "async-get-data");
    let descriptor = BundleDescriptor {
      inner: BundleDescriptorInner::Owned(bundle.descriptor().clone()),
    };
    let filepath = path.to_str().unwrap().to_string();

    let data = descriptor
      .async_get_data(filepath.clone(), "/index.html".to_string())
      .await
      .unwrap();
    assert_eq!(data.as_deref(), Some(BODY));

    let checksum = descriptor
      .async_get_data_checksum(filepath, "/index.html".to_string())
      .await
      .unwrap();
    assert_eq!(checksum, bundle.get_data_checksum("/index.html").unwrap());

    std::fs::remove_file(&path).unwrap();
  }

  /// `fetch_descriptor` hands back a metadata-only descriptor. Reading its index used to
  /// panic, which crosses the FFI boundary as an abort rather than a catchable error.
  #[test]
  fn index_is_readable_from_a_metadata_only_descriptor() {
    let bundle = bundle();
    let descriptor = BundleDescriptor {
      inner: BundleDescriptorInner::Owned(bundle.descriptor().clone()),
    };

    let index = descriptor.index();
    assert!(index.contains_path("/index.html".to_string()));
    assert_eq!(index.entries().len(), 1);
  }

  /// The cached-descriptor variant handed out by `LoadedDescriptor::descriptor`.
  #[test]
  fn index_is_readable_from_a_shared_descriptor() {
    let bundle = bundle();
    let descriptor = BundleDescriptor {
      inner: BundleDescriptorInner::Arc(Arc::new(bundle.descriptor().clone())),
    };

    assert!(descriptor.index().contains_path("/index.html".to_string()));
  }

  /// The fully loaded variant keeps working, and agrees with the metadata-only one.
  #[test]
  fn index_from_a_loaded_bundle_matches_the_descriptor() {
    let bundle = Arc::new(bundle());
    let loaded = BundleDescriptor {
      inner: BundleDescriptorInner::Bundle(bundle.clone()),
    };
    let metadata_only = BundleDescriptor {
      inner: BundleDescriptorInner::Owned(bundle.descriptor().clone()),
    };

    assert_eq!(
      loaded
        .index()
        .get_entry("/index.html".to_string())
        .unwrap()
        .content_length,
      metadata_only
        .index()
        .get_entry("/index.html".to_string())
        .unwrap()
        .content_length,
    );
  }
}
