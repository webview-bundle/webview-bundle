use crate::http::HttpHeaders;
use crate::mime::MimeType;
use crate::version::Version;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::collections::HashMap;
use std::io::Cursor;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use wvb::http::HeaderMap;
use wvb::{
  AsyncBundleReader, AsyncBundleWriter, AsyncReader, AsyncWriter, BundleBuilderOptions,
  BundleEntry, BundleReader, BundleWriter, HeaderWriterOptions, IndexWriterOptions, Reader, Writer,
};

/// Bundle header containing format metadata.
///
/// The header is the first 17 bytes of a `.wvb` file and includes:
/// - Magic number (🌐🎁)
/// - Format version
/// - Index size
/// - Header checksum
#[napi]
pub struct Header {
  pub(crate) inner: SharedReference<BundleDescriptor, &'static wvb::Header>,
}

#[napi]
impl Header {
  /// Returns the bundle format version.
  ///
  /// @returns {Version} The format version (e.g., V1)
  ///
  /// @example
  /// ```typescript
  /// const header = bundle.descriptor().header();
  /// console.log(header.version()); // 'v1'
  /// ```
  #[napi]
  pub fn version(&self) -> Version {
    Version::from(self.inner.version())
  }

  /// Returns the byte offset where the index section ends.
  ///
  /// This marks the start of the data section.
  ///
  /// @returns {bigint} Byte offset
  #[napi]
  pub fn index_end_offset(&self) -> u64 {
    self.inner.index_end_offset()
  }

  /// Returns the size of the index section in bytes.
  ///
  /// @returns {number} Index size in bytes
  #[napi]
  pub fn index_size(&self) -> u32 {
    self.inner.index_size()
  }
}

/// Metadata for a single file in the bundle.
///
/// Contains information about file location, size, MIME type, and HTTP headers.
///
/// @property {number} offset - Byte offset in the data section (compressed)
/// @property {number} len - Length of compressed data in bytes
/// @property {boolean} isEmpty - Whether the compressed data is empty
/// @property {string} contentType - MIME type of the file
/// @property {number} contentLength - Original file size before compression
/// @property {Record<string, string>} headers - HTTP headers for this file
#[napi(object)]
pub struct IndexEntry {
  pub offset: u32,
  pub len: u32,
  pub is_empty: bool,
  pub content_type: String,
  pub content_length: u32,
  pub headers: HashMap<String, String>,
}

impl From<&wvb::IndexEntry> for IndexEntry {
  fn from(value: &wvb::IndexEntry) -> Self {
    Self {
      offset: value.offset() as u32,
      len: value.len() as u32,
      is_empty: value.is_empty(),
      content_type: value.content_type().to_string(),
      content_length: value.content_length() as u32,
      headers: HttpHeaders::from(value.headers()).0,
    }
  }
}

/// Bundle index mapping file paths to their metadata.
///
/// The index is stored as binary data in the bundle file and maps file paths
/// to their metadata (offset, length, content-type, headers, etc.).
#[napi]
pub struct Index {
  pub(crate) inner: SharedReference<BundleDescriptor, &'static wvb::Index>,
}

#[napi]
impl Index {
  /// Returns all index entries as a map of path to metadata.
  ///
  /// @returns {Record<string, IndexEntry>} Map of file paths to entry metadata
  ///
  /// @example
  /// ```typescript
  /// const index = bundle.descriptor().index();
  /// const entries = index.entries();
  /// for (const [path, entry] of Object.entries(entries)) {
  ///   console.log(`${path}: ${entry.contentType}`);
  /// }
  /// ```
  #[napi]
  pub fn entries(&self) -> HashMap<String, IndexEntry> {
    let mut entries = HashMap::with_capacity(self.inner.entries().len());
    for (key, value) in self.inner.entries() {
      entries.insert(key.to_string(), IndexEntry::from(value));
    }
    entries
  }

  /// Gets the index entry for a specific file path.
  ///
  /// @param {string} path - File path in the bundle (e.g., "/index.html")
  /// @returns {IndexEntry | null} Entry metadata or null if not found
  ///
  /// @example
  /// ```typescript
  /// const entry = index.getEntry('/index.html');
  /// if (entry) {
  ///   console.log(`Content-Type: ${entry.contentType}`);
  /// }
  /// ```
  #[napi]
  pub fn get_entry(&self, path: String) -> Option<IndexEntry> {
    self.inner.get_entry(&path).map(IndexEntry::from)
  }

  /// Checks if a file path exists in the bundle.
  ///
  /// @param {string} path - File path to check
  /// @returns {boolean} True if the path exists
  ///
  /// @example
  /// ```typescript
  /// if (index.containsPath('/app.js')) {
  ///   console.log('app.js is in the bundle');
  /// }
  /// ```
  #[napi]
  pub fn contains_path(&self, path: String) -> bool {
    self.inner.contains_path(&path)
  }
}

pub(crate) enum BundleDescriptorInner {
  Owned(wvb::BundleDescriptor),
  Arc(Arc<wvb::BundleDescriptor>),
  Bundle(SharedReference<Bundle, &'static wvb::BundleDescriptor>),
}

unsafe impl Send for BundleDescriptorInner {}
unsafe impl Sync for BundleDescriptorInner {}

impl Deref for BundleDescriptorInner {
  type Target = wvb::BundleDescriptor;
  fn deref(&self) -> &Self::Target {
    match self {
      Self::Owned(x) => x,
      Self::Arc(x) => x,
      Self::Bundle(x) => x,
    }
  }
}

/// Bundle metadata including header and index information.
///
/// A descriptor contains only the metadata without loading the actual file data,
/// making it efficient for inspecting bundle contents.
///
/// @example
/// ```typescript
/// const bundle = await readBundle('app.wvb');
/// const descriptor = bundle.descriptor();
/// const header = descriptor.header();
/// const index = descriptor.index();
/// ```
#[napi]
pub struct BundleDescriptor {
  pub(crate) inner: BundleDescriptorInner,
}

#[napi]
impl BundleDescriptor {
  /// Returns the bundle header.
  ///
  /// @returns {Header} Bundle header with format metadata
  #[napi]
  pub fn header(&self, this: Reference<BundleDescriptor>, env: Env) -> crate::Result<Header> {
    let inner = this.share_with(env, |manifest| Ok(manifest.inner.header()))?;
    Ok(Header { inner })
  }

  /// Returns the bundle index.
  ///
  /// @returns {Index} Bundle index with file metadata
  #[napi]
  pub fn index(&self, this: Reference<BundleDescriptor>, env: Env) -> crate::Result<Index> {
    let inner = this.share_with(env, |manifest| Ok(manifest.inner.index()))?;
    Ok(Index { inner })
  }

  /// Read data from the bundle.
  ///
  /// @param {string} filepath - File path for the bundle
  /// @param {string} path - Path to the bundle data
  /// @returns {Buffer | null} Data from the bundle or `null` if not found
  #[napi]
  pub fn get_data(&self, filepath: String, path: String) -> crate::Result<Option<Buffer>> {
    let file = Self::open_file(&filepath)?;
    let data = self.inner.get_data(file, &path)?;
    Ok(data.map(Buffer::from))
  }

  /// Read checksum from the bundle.
  ///
  /// @param {string} filepath - File path for the bundle
  /// @param {string} path - Path to the bundle data
  /// @returns {number | null} Checksum for the bundle data or `null` if not found
  #[napi]
  pub fn get_data_checksum(&self, filepath: String, path: String) -> crate::Result<Option<u32>> {
    let file = Self::open_file(&filepath)?;
    let checksum = self.inner.get_data_checksum(file, &path)?;
    Ok(checksum)
  }

  /// Asynchronously read data from the bundle.
  ///
  /// @param {string} filepath - File path for the bundle
  /// @param {string} path - Path to the bundle data
  /// @returns {Promise<Buffer | null>} Data from the bundle or `null` if not found
  #[napi]
  pub async fn async_get_data(
    &self,
    filepath: String,
    path: String,
  ) -> crate::Result<Option<Buffer>> {
    let file = Self::async_open_file(&filepath).await?;
    let data = self.inner.async_get_data(file, &path).await?;
    Ok(data.map(Buffer::from))
  }

  /// Asynchronously read checksum from the bundle.
  ///
  /// @param {string} filepath - File path for the bundle
  /// @param {string} path - Path to the bundle data
  /// @returns {Promise<number | null>} Checksum for the bundle data or `null` if not found
  #[napi]
  pub async fn async_get_data_checksum(
    &self,
    filepath: String,
    path: String,
  ) -> crate::Result<Option<u32>> {
    let file = Self::async_open_file(&filepath).await?;
    let checksum = self.inner.async_get_data_checksum(file, &path).await?;
    Ok(checksum)
  }

  fn open_file(filepath: &str) -> crate::Result<std::fs::File> {
    std::fs::File::open(Path::new(filepath)).map_err(|e| crate::Error::Core(wvb::Error::Io(e)))
  }

  async fn async_open_file(filepath: &str) -> crate::Result<fs::File> {
    fs::File::open(Path::new(filepath))
      .await
      .map_err(|e| crate::Error::Core(wvb::Error::Io(e)))
  }
}

/// A complete bundle including metadata and file data.
///
/// Represents a `.wvb` bundle file loaded entirely into memory.
/// Use this when you need to access multiple files or build new bundles.
///
/// @example
/// ```typescript
/// // Read a bundle from file
/// const bundle = await readBundle('app.wvb');
///
/// // Access files
/// const html = bundle.getData('/index.html');
/// if (html) {
///   console.log(html.toString('utf-8'));
/// }
/// ```
#[napi]
pub struct Bundle {
  pub(crate) inner: wvb::Bundle,
}

#[napi]
impl Bundle {
  /// Returns the bundle descriptor (header and index).
  ///
  /// @returns {BundleDescriptor} Bundle metadata
  ///
  /// @example
  /// ```typescript
  /// const descriptor = bundle.descriptor();
  /// const index = descriptor.index();
  /// ```
  #[napi]
  pub fn descriptor(&self, this: Reference<Bundle>, env: Env) -> crate::Result<BundleDescriptor> {
    let inner = this.share_with(env, |bundle| Ok(bundle.inner.descriptor()))?;
    Ok(BundleDescriptor {
      inner: BundleDescriptorInner::Bundle(inner),
    })
  }

  /// Retrieves file data by path.
  ///
  /// Returns the decompressed file contents, or null if the path doesn't exist.
  ///
  /// @param {string} path - File path in the bundle (e.g., "/index.html")
  /// @returns {Buffer | null} File contents or null if not found
  ///
  /// @example
  /// ```typescript
  /// const data = bundle.getData('/index.html');
  /// if (data) {
  ///   console.log(data.toString('utf-8'));
  /// }
  /// ```
  #[napi]
  pub fn get_data(&self, path: String) -> crate::Result<Option<Buffer>> {
    let buf = self.inner.get_data(&path)?.map(|x| x.into());
    Ok(buf)
  }

  /// Retrieves the checksum of file data by path.
  ///
  /// @param {string} path - File path in the bundle
  /// @returns {number | null} xxHash-32 checksum or null if not found
  #[napi]
  pub fn get_data_checksum(&self, path: String) -> crate::Result<Option<u32>> {
    let checksum = self.inner.get_data_checksum(&path)?;
    Ok(checksum)
  }
}

/// Reads a bundle from a buffer synchronously.
///
/// @param {Buffer} buffer - Buffer containing bundle data
/// @returns {Bundle} Parsed bundle
/// @throws {Error} If the buffer is not a valid bundle
///
/// @example
/// ```typescript
/// import { readFileSync } from 'fs';
/// const buffer = readFileSync('app.wvb');
/// const bundle = readBundleFromBuffer(buffer);
/// ```
#[napi]
pub fn read_bundle_from_buffer(buffer: BufferSlice) -> crate::Result<Bundle> {
  let cursor = Cursor::new(buffer.as_ref());
  let bundle = Reader::<wvb::Bundle>::read(&mut BundleReader::new(cursor))?;
  Ok(Bundle { inner: bundle })
}

/// Reads a bundle from a file asynchronously.
///
/// @param {string} filepath - Path to the `.wvb` file
/// @returns {Promise<Bundle>} Parsed bundle
/// @throws {Error} If the file cannot be read or is not a valid bundle
///
/// @example
/// ```typescript
/// const bundle = await readBundle('app.wvb');
/// const html = bundle.getData('/index.html');
/// ```
#[napi]
pub async fn read_bundle(filepath: String) -> crate::Result<Bundle> {
  let mut file = fs::File::open(&filepath)
    .await
    .map_err(|e| crate::Error::Core(wvb::Error::from(e)))?;
  let bundle = AsyncReader::<wvb::Bundle>::read(&mut AsyncBundleReader::new(&mut file)).await?;
  Ok(Bundle { inner: bundle })
}

/// Writes a bundle to a file asynchronously.
///
/// @param {Bundle} bundle - Bundle to write
/// @param {string} filepath - Destination file path
/// @returns {Promise<number>} Number of bytes written
/// @throws {Error} If the file cannot be written
///
/// @example
/// ```typescript
/// const builder = new BundleBuilder();
/// builder.insertEntry('/index.html', Buffer.from('<html></html>'));
/// const bundle = builder.build();
/// await writeBundle(bundle, 'output.wvb');
/// ```
#[napi]
pub async fn write_bundle(bundle: &Bundle, filepath: String) -> crate::Result<usize> {
  let mut file = fs::File::create(&filepath)
    .await
    .map_err(|e| crate::Error::Core(wvb::Error::from(e)))?;
  let size =
    AsyncWriter::<wvb::Bundle>::write(&mut AsyncBundleWriter::new(&mut file), &bundle.inner)
      .await?;
  Ok(size)
}

/// Writes a bundle to a buffer synchronously.
///
/// @param {Bundle} bundle - Bundle to write
/// @returns {Buffer} Bundle data as a buffer
///
/// @example
/// ```typescript
/// const bundle = builder.build();
/// const buffer = writeBundleIntoBuffer(bundle);
/// console.log(`Bundle size: ${buffer.length} bytes`);
/// ```
#[napi]
pub fn write_bundle_into_buffer(bundle: &Bundle) -> crate::Result<Buffer> {
  let mut buf = vec![];
  Writer::<wvb::Bundle>::write(&mut BundleWriter::new(&mut buf), &bundle.inner)?;
  Ok(buf.into())
}

/// Checksum verification for the data section.
///
/// @property {boolean} [verify] - Verify the section's xxHash-32 checksum on read (default: true)
/// @property {number} [seed] - Seed the checksum was built with (default: 0)
#[napi(object)]
pub struct ChecksumReadOptions {
  pub verify: Option<bool>,
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

/// How entry data is read out of a bundle's data section.
///
/// @property {ChecksumReadOptions} [checksum] - Checksum verification for the data section
#[napi(object)]
pub struct DataReadOptions {
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

/// How a bundle's header is read.
///
/// @property {ChecksumReadOptions} [checksum] - Checksum verification for the header section
#[napi(object)]
pub struct HeaderReadOptions {
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

/// How a bundle's index is read.
///
/// @property {ChecksumReadOptions} [checksum] - Checksum verification for the index section
#[napi(object)]
pub struct IndexReadOptions {
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

/// Options for building a bundle.
///
/// @property {BuildHeaderOptions} [header] - Header generation options
/// @property {BuildIndexOptions} [index] - Index generation options
/// @property {ChecksumWriteOptions} [dataChecksum] - Checksum generation for the data section
#[napi(object)]
pub struct BuildOptions {
  pub header: Option<BuildHeaderOptions>,
  pub index: Option<BuildIndexOptions>,
  pub data_checksum: Option<ChecksumWriteOptions>,
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

/// Checksum generation for a bundle section.
///
/// @property {number} [seed] - Seed the checksum is built with (default: 0)
#[napi(object)]
pub struct ChecksumWriteOptions {
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

/// Options for bundle header generation.
///
/// @property {ChecksumWriteOptions} [checksum] - Checksum generation for the header section
#[napi(object)]
pub struct BuildHeaderOptions {
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

/// Options for bundle index generation.
///
/// @property {ChecksumWriteOptions} [checksum] - Checksum generation for the index section
#[napi(object)]
pub struct BuildIndexOptions {
  pub checksum: Option<ChecksumWriteOptions>,
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

/// Builder for creating bundle files.
///
/// Allows you to add files, set options, and generate a complete bundle.
///
/// @example
/// ```typescript
/// const builder = new BundleBuilder();
///
/// // Add files
/// builder.insertEntry('/index.html', Buffer.from('<html>...</html>'));
/// builder.insertEntry('/app.js', Buffer.from("console.log('hello');"));
///
/// // Build the bundle
/// const bundle = builder.build();
///
/// // Write to file
/// await writeBundle(bundle, 'app.wvb');
/// ```
#[napi]
pub struct BundleBuilder {
  pub(crate) version: Version,
  pub(crate) inner: wvb::BundleBuilder,
}

#[napi]
impl BundleBuilder {
  /// Creates a new bundle builder.
  ///
  /// @param {Version} [version] - Bundle format version (defaults to V1)
  ///
  /// @example
  /// ```typescript
  /// const builder = new BundleBuilder();
  /// ```
  #[napi(constructor)]
  pub fn new(version: Option<Version>) -> BundleBuilder {
    Self {
      version: version.unwrap_or(Version::V1),
      inner: wvb::BundleBuilder::new(),
    }
  }

  /// Gets the bundle format version.
  ///
  /// @returns {Version} Bundle format version
  #[napi(getter)]
  pub fn version(&self) -> &Version {
    &self.version
  }

  /// Returns all entry paths currently in the builder.
  ///
  /// @returns {string[]} Array of file paths
  ///
  /// @example
  /// ```typescript
  /// const paths = builder.entryPaths();
  /// console.log(paths); // ["/index.html", "/app.js"]
  /// ```
  #[napi]
  pub fn entry_paths(&self) -> Vec<String> {
    self.inner.entries().keys().map(|s| s.to_string()).collect()
  }

  /// Adds or updates a file in the bundle.
  ///
  /// If `contentType` is not provided, it will be auto-detected from the file
  /// extension and content.
  ///
  /// @param {string} path - File path (must start with "/")
  /// @param {Buffer} data - File contents
  /// @param {string} [contentType] - MIME type (auto-detected if not provided)
  /// @param {Record<string, string>} [headers] - Optional HTTP headers
  /// @returns {boolean} True if a file was replaced, false if newly added
  ///
  /// @example
  /// ```typescript
  /// // Auto-detect MIME type
  /// builder.insertEntry('/index.html', Buffer.from('<html></html>'));
  ///
  /// // Specify MIME type
  /// builder.insertEntry('/data.bin', buffer, 'application/octet-stream');
  ///
  /// // With custom headers
  /// builder.insertEntry('/style.css', cssBuffer, 'text/css', {
  ///   'Cache-Control': 'max-age=3600',
  /// });
  /// ```
  #[napi]
  pub fn insert_entry(
    &mut self,
    path: String,
    data: Buffer,
    content_type: Option<String>,
    headers: Option<HashMap<String, String>>,
  ) -> crate::Result<bool> {
    let headers = if let Some(h) = headers {
      Some(HeaderMap::try_from(HttpHeaders::from(h))?)
    } else {
      None
    };
    let content_type = content_type.unwrap_or_else(|| {
      let mime = MimeType::parse_with_fallback(data.as_ref(), &path, MimeType::OctetStream);
      mime.to_string()
    });
    Ok(
      self
        .inner
        .insert_entry(path, BundleEntry::new(data.as_ref(), content_type, headers))
        .is_some(),
    )
  }

  /// Removes a file from the bundle.
  ///
  /// @param {string} path - File path to remove
  /// @returns {boolean} True if the file was removed, false if not found
  ///
  /// @example
  /// ```typescript
  /// builder.removeEntry('/old-file.js');
  /// ```
  #[napi]
  pub fn remove_entry(&mut self, path: String) -> bool {
    self.inner.remove_entry(&path).is_some()
  }

  /// Checks if a file path exists in the builder.
  ///
  /// @param {string} path - File path to check
  /// @returns {boolean} True if the path exists
  ///
  /// @example
  /// ```typescript
  /// if (builder.containsEntry('/index.html')) {
  ///   console.log('index.html already added');
  /// }
  /// ```
  #[napi]
  pub fn contains_entry(&self, path: String) -> bool {
    self.inner.contains_path(&path)
  }

  /// Builds the bundle with all added files.
  ///
  /// This consumes the builder's entries and creates a complete bundle with
  /// compressed data.
  ///
  /// @param {BuildOptions} [options] - Build options
  /// @returns {Bundle} The built bundle
  ///
  /// @example
  /// ```typescript
  /// const bundle = builder.build();
  /// await writeBundle(bundle, 'output.wvb');
  /// ```
  #[napi]
  pub fn build(&mut self, options: Option<BuildOptions>) -> crate::Result<Bundle> {
    if let Some(options) = options {
      self.inner.set_options(options.into());
    }
    let bundle = self.inner.build()?;
    Ok(Bundle { inner: bundle })
  }
}
