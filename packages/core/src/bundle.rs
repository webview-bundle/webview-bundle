use crate::builder::BundleBuilder;
use crate::checksum::{CHECKSUM_LEN, make_checksum, parse_checksum};
use crate::header::{Header, HeaderReadOptions, HeaderReader, HeaderWriter};
use crate::index::{Index, IndexEntry, IndexReadOptions, IndexReader, IndexWriter};
use crate::reader::Reader;
use crate::writer::Writer;
use lz4_flex::decompress_size_prepended;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use crate::ChecksumReadOptions;
#[cfg(feature = "async")]
use crate::{
  AsyncHeaderReader, AsyncHeaderWriter, AsyncIndexReader, AsyncIndexWriter, AsyncReader,
  AsyncWriter,
};
#[cfg(feature = "async")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt};

/// How entry data is read out of a bundle's data section.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DataReadOptions {
  pub checksum: ChecksumReadOptions,
}

impl DataReadOptions {
  pub fn checksum(mut self, checksum: ChecksumReadOptions) -> Self {
    self.checksum = checksum;
    self
  }
}

/// Bundle metadata including header and index information.
///
/// A `BundleDescriptor` contains the header and index of a bundle without loading
/// the full file data. This is useful for:
///
/// - Reading bundle metadata without loading all files
/// - Lazy-loading files on demand from a reader
/// - Inspecting bundle contents efficiently
///
/// # Example
///
/// ```no_run
/// # use wvb::{AsyncBundleReader, AsyncReader, BundleDescriptor};
/// # async {
/// # use tokio::fs::File;
/// let mut file = File::open("app.wvb").await.unwrap();
/// let mut reader = AsyncBundleReader::new(&mut file);
/// let descriptor = AsyncReader::<BundleDescriptor>::read(&mut reader).await.unwrap();
///
/// // Check if file exists
/// if descriptor.index().contains_path("/index.html") {
///     // Load file on demand
///     let data = descriptor.async_get_data(&mut file, "/index.html").await.unwrap();
/// }
/// # };
/// ```
#[derive(Debug, PartialEq, Clone)]
pub struct BundleDescriptor {
  pub(crate) header: Header,
  pub(crate) index: Index,
}

impl BundleDescriptor {
  /// Returns a reference to the bundle header.
  pub fn header(&self) -> &Header {
    &self.header
  }

  /// Returns a reference to the bundle index.
  pub fn index(&self) -> &Index {
    &self.index
  }

  /// Reads the data from the bundle using the provided reader.
  ///
  /// Returns `None` if the path doesn't exist in the bundle.
  ///
  /// # Arguments
  ///
  /// * `reader` - A reader positioned at the start of the bundle file
  /// * `path` - File path in the bundle (e.g., "/index.html")
  pub fn get_data<R: Read + Seek>(&self, reader: R, path: &str) -> crate::Result<Option<Vec<u8>>> {
    self.get_data_with_options(reader, path, DataReadOptions::default())
  }

  /// Reads the data from the bundle with options.
  ///
  /// Returns `None` if the path doesn't exist in the bundle, and
  /// [`crate::Error::ChecksumMismatch`] if the entry's data is corrupted.
  pub fn get_data_with_options<R: Read + Seek>(
    &self,
    reader: R,
    path: &str,
    options: DataReadOptions,
  ) -> crate::Result<Option<Vec<u8>>> {
    if !self.index.contains_path(path) {
      return Ok(None);
    }
    let entry = self.index.get_entry(path).unwrap();
    let mut reader =
      BundleDataReader::new_with_options(reader, self.header.index_end_offset(), options);
    let data = reader.read_entry_data(entry)?;
    Ok(Some(data))
  }

  /// Reads the checksum of file data from the bundle.
  ///
  /// Returns `None` if the path doesn't exist in the bundle.
  pub fn get_data_checksum<R: Read + Seek>(
    &self,
    reader: R,
    path: &str,
  ) -> crate::Result<Option<u32>> {
    if !self.index.contains_path(path) {
      return Ok(None);
    }
    let entry = self.index.get_entry(path).unwrap();
    let mut reader = BundleDataReader::new(reader, self.header.index_end_offset());
    let checksum = reader.read_entry_checksum(entry)?;
    Ok(Some(checksum))
  }

  /// Asynchronously reads the data from the bundle.
  ///
  /// Returns `None` if the path doesn't exist in the bundle.
  #[cfg(feature = "async")]
  pub async fn async_get_data<R: AsyncRead + AsyncSeek + Unpin>(
    &self,
    reader: R,
    path: &str,
  ) -> crate::Result<Option<Vec<u8>>> {
    self
      .async_get_data_with_options(reader, path, DataReadOptions::default())
      .await
  }

  /// Asynchronously reads the data from the bundle.
  ///
  /// Returns `None` if the path doesn't exist in the bundle, and
  /// [`crate::Error::ChecksumMismatch`] if the entry's data is corrupted.
  #[cfg(feature = "async")]
  pub async fn async_get_data_with_options<R: AsyncRead + AsyncSeek + Unpin>(
    &self,
    reader: R,
    path: &str,
    options: DataReadOptions,
  ) -> crate::Result<Option<Vec<u8>>> {
    if !self.index.contains_path(path) {
      return Ok(None);
    }
    let entry = self.index.get_entry(path).unwrap();
    let mut reader =
      AsyncBundleDataReader::new_with_options(reader, self.header.index_end_offset(), options);
    let data = reader.read_entry_data(entry).await?;
    Ok(Some(data))
  }

  /// Asynchronously reads the checksum of file data from the bundle.
  ///
  /// Returns `None` if the path doesn't exist in the bundle.
  #[cfg(feature = "async")]
  pub async fn async_get_data_checksum<R: AsyncRead + AsyncSeek + Unpin>(
    &self,
    reader: R,
    path: &str,
  ) -> crate::Result<Option<u32>> {
    if !self.index.contains_path(path) {
      return Ok(None);
    }
    let entry = self.index.get_entry(path).unwrap();
    let mut reader = AsyncBundleDataReader::new(reader, self.header.index_end_offset());
    let data = reader.read_entry_checksum(entry).await?;
    Ok(Some(data))
  }
}

/// A complete bundle including metadata and file data.
///
/// A `Bundle` contains all the data from a `.wvb` file in memory. Use this when:
///
/// - You need to access multiple files frequently
/// - The bundle is small enough to fit in memory
/// - You're building a new bundle to write to disk
///
/// For large bundles or when you only need a few files, consider using
/// `BundleDescriptor` instead to load files on demand.
///
/// # Example
///
/// ```no_run
/// # use wvb::{AsyncBundleReader, AsyncReader, Bundle};
/// # async {
/// # use tokio::fs::File;
/// // Read entire bundle into memory
/// let mut file = File::open("app.wvb").await.unwrap();
/// let mut reader = AsyncBundleReader::new(&mut file);
/// let bundle = AsyncReader::<Bundle>::read(&mut reader).await.unwrap();
///
/// // Access files directly
/// let html = bundle.get_data("/index.html").unwrap().unwrap();
/// let css = bundle.get_data("/style.css").unwrap().unwrap();
/// # };
/// ```
#[derive(Debug, PartialEq, Clone)]
pub struct Bundle {
  pub(crate) descriptor: BundleDescriptor,
  pub(crate) data: Vec<u8>,
}

impl Bundle {
  /// Creates a new bundle builder.
  ///
  /// # Example
  ///
  /// ```
  /// use wvb::{Bundle, BundleEntry};
  ///
  /// let mut builder = Bundle::builder();
  /// builder.insert_entry(
  ///     "/index.html",
  ///     BundleEntry::new(b"<html></html>", "text/html", None),
  /// );
  /// let bundle = builder.build().unwrap();
  /// ```
  pub fn builder() -> BundleBuilder {
    BundleBuilder::new()
  }

  /// Creates a new bundle builder with pre-allocated capacity.
  ///
  /// Use this when you know approximately how many files you'll add.
  pub fn builder_with_capacity(capacity: usize) -> BundleBuilder {
    BundleBuilder::new_with_capacity(capacity)
  }

  /// Returns a reference to the bundle descriptor (header and index).
  pub fn descriptor(&self) -> &BundleDescriptor {
    &self.descriptor
  }

  /// Retrieves file data by path.
  ///
  /// Returns `None` if the path doesn't exist in the bundle.
  ///
  /// # Example
  ///
  /// ```
  /// # use wvb::{Bundle, BundleEntry};
  /// let mut builder = Bundle::builder();
  /// builder.insert_entry("/test.txt", BundleEntry::new(b"hello", "text/plain", None));
  /// let bundle = builder.build().unwrap();
  ///
  /// let data = bundle.get_data("/test.txt").unwrap().unwrap();
  /// assert_eq!(data, b"hello");
  /// ```
  pub fn get_data(&self, path: &str) -> crate::Result<Option<Vec<u8>>> {
    self.get_data_with_options(path, DataReadOptions::default())
  }

  /// Retrieves file data by path, using the checksum policy in [`DataReadOptions`].
  ///
  /// Returns `None` if the path doesn't exist in the bundle, and
  /// [`crate::Error::ChecksumMismatch`] if the entry's data is corrupted.
  pub fn get_data_with_options(
    &self,
    path: &str,
    options: DataReadOptions,
  ) -> crate::Result<Option<Vec<u8>>> {
    if !self.descriptor.index.contains_path(path) {
      return Ok(None);
    }
    let entry = self.descriptor.index.get_entry(path).unwrap();
    let mut reader = BundleDataReader::new_with_options(Cursor::new(&self.data), 0, options);
    let data = reader.read_entry_data(entry)?;
    Ok(Some(data))
  }

  /// Retrieves the checksum of file data by path.
  ///
  /// Returns `None` if the path doesn't exist in the bundle.
  pub fn get_data_checksum(&self, path: &str) -> crate::Result<Option<u32>> {
    if !self.descriptor.index.contains_path(path) {
      return Ok(None);
    }
    let entry = self.descriptor.index.get_entry(path).unwrap();
    let mut reader = BundleDataReader::new(Cursor::new(&self.data), 0);
    let checksum = reader.read_entry_checksum(entry)?;
    Ok(Some(checksum))
  }
}

fn read_entry(entry: &IndexEntry, options: &DataReadOptions) -> (u64, Vec<u8>) {
  let len = entry.len() as usize;
  // When the checksum is to be verified the buffer also covers the 4-byte checksum.
  let len = match options.checksum.verify {
    true => len + CHECKSUM_LEN,
    false => len,
  };
  (entry.offset(), vec![0u8; len])
}

fn parse_entry(
  buf: &[u8],
  entry: &IndexEntry,
  options: &DataReadOptions,
) -> crate::Result<Vec<u8>> {
  let data = match options.checksum.verify {
    true => {
      let (data, checksum) = buf.split_at(entry.len() as usize);
      if make_checksum(options.checksum.seed, data) != parse_checksum(checksum) {
        return Err(crate::Error::ChecksumMismatch);
      }
      data
    }
    false => buf,
  };
  let decompressed = decompress_size_prepended(data)?;
  Ok(decompressed)
}

fn read_entry_checksum(entry: &IndexEntry) -> (u64, [u8; CHECKSUM_LEN]) {
  (entry.offset() + entry.len(), [0u8; CHECKSUM_LEN])
}

pub(crate) struct BundleDataReader<R: Read + Seek> {
  r: R,
  base_offset: u64,
  options: DataReadOptions,
}

impl<R: Read + Seek> BundleDataReader<R> {
  pub fn new(r: R, base_offset: u64) -> Self {
    Self::new_with_options(r, base_offset, Default::default())
  }

  pub fn new_with_options(r: R, base_offset: u64, options: DataReadOptions) -> Self {
    Self {
      r,
      base_offset,
      options,
    }
  }

  pub fn read_entry_data(&mut self, entry: &IndexEntry) -> crate::Result<Vec<u8>> {
    let (offset, mut buf) = read_entry(entry, &self.options);
    self.r.seek(SeekFrom::Start(self.base_offset + offset))?;
    self.r.read_exact(&mut buf)?;
    parse_entry(&buf, entry, &self.options)
  }

  pub fn read_entry_checksum(&mut self, entry: &IndexEntry) -> crate::Result<u32> {
    let (offset, mut buf) = read_entry_checksum(entry);
    self.r.seek(SeekFrom::Start(self.base_offset + offset))?;
    self.r.read_exact(&mut buf)?;
    Ok(parse_checksum(&buf))
  }
}

#[cfg(feature = "async")]
pub(crate) struct AsyncBundleDataReader<R: AsyncRead + AsyncSeek + Unpin> {
  r: R,
  base_offset: u64,
  options: DataReadOptions,
}

#[cfg(feature = "async")]
impl<R: AsyncRead + AsyncSeek + Unpin> AsyncBundleDataReader<R> {
  pub fn new(r: R, base_offset: u64) -> Self {
    Self::new_with_options(r, base_offset, Default::default())
  }

  pub fn new_with_options(r: R, base_offset: u64, options: DataReadOptions) -> Self {
    Self {
      r,
      base_offset,
      options,
    }
  }

  pub async fn read_entry_data(&mut self, entry: &IndexEntry) -> crate::Result<Vec<u8>> {
    let (offset, mut buf) = read_entry(entry, &self.options);
    self
      .r
      .seek(SeekFrom::Start(self.base_offset + offset))
      .await?;
    self.r.read_exact(&mut buf).await?;
    parse_entry(&buf, entry, &self.options)
  }

  pub async fn read_entry_checksum(&mut self, entry: &IndexEntry) -> crate::Result<u32> {
    let (offset, mut buf) = read_entry_checksum(entry);
    self
      .r
      .seek(SeekFrom::Start(self.base_offset + offset))
      .await?;
    self.r.read_exact(&mut buf).await?;
    Ok(parse_checksum(&buf))
  }
}

pub struct BundleReader<R: Read + Seek> {
  r: R,
  header_options: HeaderReadOptions,
  index_options: IndexReadOptions,
}

impl<R: Read + Seek> BundleReader<R> {
  pub fn new(r: R) -> Self {
    Self::new_with_options(r, Default::default(), Default::default())
  }

  pub fn new_with_options(
    r: R,
    header_options: HeaderReadOptions,
    index_options: IndexReadOptions,
  ) -> Self {
    Self {
      r,
      header_options,
      index_options,
    }
  }

  pub fn read_header(&mut self) -> crate::Result<Header> {
    let mut reader = HeaderReader::new_with_options(&mut self.r, self.header_options);
    let header = reader.read()?;
    Ok(header)
  }

  pub fn read_index(&mut self, header: Header) -> crate::Result<Index> {
    let mut reader = IndexReader::new_with_options(&mut self.r, header, self.index_options);
    let index = reader.read()?;
    Ok(index)
  }

  pub fn read_data(&mut self, header: Header) -> crate::Result<Vec<u8>> {
    self.r.seek(SeekFrom::Start(header.index_end_offset()))?;
    let mut data = vec![];
    self.r.read_to_end(&mut data)?;
    Ok(data)
  }
}

impl<R: Read + Seek> Reader<BundleDescriptor> for BundleReader<R> {
  fn read(&mut self) -> crate::Result<BundleDescriptor> {
    let header = self.read_header()?;
    let index = self.read_index(header)?;
    Ok(BundleDescriptor { header, index })
  }
}

impl<R: Read + Seek> Reader<Bundle> for BundleReader<R> {
  fn read(&mut self) -> crate::Result<Bundle> {
    let header = self.read_header()?;
    let index = self.read_index(header)?;
    let data = self.read_data(header)?;
    Ok(Bundle {
      descriptor: BundleDescriptor { header, index },
      data,
    })
  }
}

#[cfg(feature = "async")]
pub struct AsyncBundleReader<R: AsyncRead + AsyncSeek + Unpin> {
  r: R,
  header_options: HeaderReadOptions,
  index_options: IndexReadOptions,
}

#[cfg(feature = "async")]
impl<R: AsyncRead + AsyncSeek + Unpin> AsyncBundleReader<R> {
  pub fn new(r: R) -> Self {
    Self::new_with_options(r, Default::default(), Default::default())
  }

  pub fn new_with_options(
    r: R,
    header_options: HeaderReadOptions,
    index_options: IndexReadOptions,
  ) -> Self {
    Self {
      r,
      header_options,
      index_options,
    }
  }

  pub async fn read_header(&mut self) -> crate::Result<Header> {
    let mut reader = AsyncHeaderReader::new_with_options(&mut self.r, self.header_options);
    let header = reader.read().await?;
    Ok(header)
  }

  pub async fn read_index(&mut self, header: Header) -> crate::Result<Index> {
    let mut reader = AsyncIndexReader::new_with_options(&mut self.r, header, self.index_options);
    let index = reader.read().await?;
    Ok(index)
  }

  pub async fn read_data(&mut self, header: Header) -> crate::Result<Vec<u8>> {
    self
      .r
      .seek(SeekFrom::Start(header.index_end_offset()))
      .await?;
    let mut data = vec![];
    self.r.read_to_end(&mut data).await?;
    Ok(data)
  }
}

#[cfg(feature = "async")]
impl<R: AsyncRead + AsyncSeek + Unpin> AsyncReader<BundleDescriptor> for AsyncBundleReader<R> {
  async fn read(&mut self) -> crate::Result<BundleDescriptor> {
    let header = self.read_header().await?;
    let index = self.read_index(header).await?;
    Ok(BundleDescriptor { header, index })
  }
}

#[cfg(feature = "async")]
impl<R: AsyncRead + AsyncSeek + Unpin> AsyncReader<Bundle> for AsyncBundleReader<R> {
  async fn read(&mut self) -> crate::Result<Bundle> {
    let header = self.read_header().await?;
    let index = self.read_index(header).await?;
    let data = self.read_data(header).await?;
    Ok(Bundle {
      descriptor: BundleDescriptor { header, index },
      data,
    })
  }
}

pub struct BundleWriter<W: Write> {
  w: W,
}

impl<W: Write> BundleWriter<W> {
  pub fn new(w: W) -> Self {
    Self { w }
  }
}

impl<W: Write> Writer<Bundle> for BundleWriter<W> {
  fn write(&mut self, data: &Bundle) -> crate::Result<usize> {
    let header_len = HeaderWriter::new(&mut self.w).write(&data.descriptor.header)?;
    let index_len = IndexWriter::new(&mut self.w).write(&data.descriptor.index)?;
    let data_len = data.data.len();
    self.w.write_all(&data.data)?;
    self.w.flush()?;
    Ok(header_len + index_len + data_len)
  }
}

#[cfg(feature = "async")]
pub struct AsyncBundleWriter<W: AsyncWrite + Unpin> {
  w: W,
}

#[cfg(feature = "async")]
impl<W: AsyncWrite + Unpin> AsyncBundleWriter<W> {
  pub fn new(w: W) -> Self {
    Self { w }
  }
}

#[cfg(feature = "async")]
impl<W: AsyncWrite + Unpin> AsyncWriter<Bundle> for AsyncBundleWriter<W> {
  async fn write(&mut self, data: &Bundle) -> crate::Result<usize> {
    let header_len = AsyncHeaderWriter::new(&mut self.w)
      .write(&data.descriptor.header)
      .await?;
    let index_len = AsyncIndexWriter::new(&mut self.w)
      .write(&data.descriptor.index)
      .await?;
    let data_len = data.data.len();
    self.w.write_all(&data.data).await?;
    self.w.flush().await?;
    Ok(header_len + index_len + data_len)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::version::Version;
  use crate::{BundleBuilderOptions, BundleEntry, ChecksumWriteOptions};
  use http::{HeaderMap, header};
  use std::io::Cursor;

  const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
  <title>test</title>
</head>
<body>
  <h1>Hello World</h1>
</body>
</html>
"#;
  const INDEX_JS: &str = r#"console.log('Hello World');"#;

  #[test]
  fn descriptor() {
    let mut builder = Bundle::builder();
    builder.insert_entry(
      "/index.html",
      BundleEntry::new(INDEX_HTML.as_bytes(), "text/html", None),
    );
    let bundle = builder.build().unwrap();
    let mut data = vec![];
    let mut writer = BundleWriter::new(Cursor::new(&mut data));
    let size = writer.write(&bundle).unwrap();
    assert_eq!(size, 150);
    let mut reader = BundleReader::new(Cursor::new(&data));
    let descriptor: BundleDescriptor = reader.read().unwrap();
    assert_eq!(descriptor.header.version(), Version::V1);
    assert_eq!(descriptor.header.index_size(), 27);

    let html = descriptor.index.get_entry("/index.html").unwrap();
    assert_eq!(html.content_type(), "text/html");
    assert_eq!(html.content_length(), INDEX_HTML.len() as u64);
    assert_eq!(html.offset(), 0);
    assert_eq!(html.len(), 98);
  }

  #[test]
  fn get_data() {
    let mut builder = Bundle::builder();
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/html".parse().unwrap());
    builder.insert_entry(
      "/index.html",
      BundleEntry::new(INDEX_HTML.as_bytes(), "text/html", Some(headers)),
    );
    builder.insert_entry(
      "/index.js",
      BundleEntry::new(INDEX_JS.as_bytes(), "text/javascript", None),
    );
    let bundle = builder.build().unwrap();
    let mut data = vec![];
    let mut writer = BundleWriter::new(Cursor::new(&mut data));
    let size = writer.write(&bundle).unwrap();
    assert_eq!(size, 240);
    let mut reader = BundleReader::new(Cursor::new(&data));
    let bundle: Bundle = reader.read().unwrap();

    let html = bundle.get_data("/index.html").unwrap().unwrap();
    assert_eq!(html, INDEX_HTML.as_bytes());

    let js = bundle.get_data("/index.js").unwrap().unwrap();
    assert_eq!(js, INDEX_JS.as_bytes());

    // Not found
    assert!(bundle.get_data("/not_found.html").unwrap().is_none());
  }

  #[test]
  fn serialization_is_deterministic_regardless_of_insert_order() {
    fn serialize(pairs: &[(&str, &str)]) -> Vec<u8> {
      let mut builder = Bundle::builder();
      for (path, body) in pairs {
        builder.insert_entry(*path, BundleEntry::new(body.as_bytes(), "text/plain", None));
      }
      let bundle = builder.build().unwrap();
      let mut data = vec![];
      BundleWriter::new(Cursor::new(&mut data))
        .write(&bundle)
        .unwrap();
      data
    }
    let forward = serialize(&[
      ("/a.txt", "aaa"),
      ("/b.txt", "bbb"),
      ("/c.txt", "ccc"),
      ("/d.txt", "ddd"),
    ]);
    let reverse = serialize(&[
      ("/d.txt", "ddd"),
      ("/c.txt", "ccc"),
      ("/b.txt", "bbb"),
      ("/a.txt", "aaa"),
    ]);
    assert_eq!(
      forward, reverse,
      "bundle bytes must be identical regardless of insertion order"
    );

    let mut reader = BundleReader::new(Cursor::new(&forward));
    let parsed: Bundle = reader.read().unwrap();
    let mut reserialized = vec![];
    BundleWriter::new(Cursor::new(&mut reserialized))
      .write(&parsed)
      .unwrap();
    assert_eq!(
      forward, reserialized,
      "re-serializing a parsed bundle must reproduce identical bytes"
    );
  }

  /// The data section stores each entry as `[compressed bytes][4-byte checksum]`, so the
  /// checksum for the entry at `path` starts right after its compressed bytes.
  fn checksum_offset(bundle: &Bundle, path: &str) -> usize {
    let entry = bundle.descriptor().index().get_entry(path).unwrap();
    (entry.offset() + entry.len()) as usize
  }

  fn bundle_with_seed(seed: u32) -> Bundle {
    let options =
      BundleBuilderOptions::default().data_checksum(ChecksumWriteOptions::default().seed(seed));
    let mut builder = BundleBuilder::new_with_options(options);
    builder.insert_entry(
      "/index.html",
      BundleEntry::new(INDEX_HTML.as_bytes(), "text/html", None),
    );
    builder.build().unwrap()
  }

  fn verifying(seed: u32) -> DataReadOptions {
    DataReadOptions::default().checksum(ChecksumReadOptions::default().verify(true).seed(seed))
  }

  #[test]
  fn get_data_verifies_checksum() {
    let bundle = bundle_with_seed(0);
    let html = bundle
      .get_data_with_options("/index.html", verifying(0))
      .unwrap();
    assert_eq!(html.unwrap(), INDEX_HTML.as_bytes());
  }

  /// The default read options verify the checksum with the default seed `0`.
  #[test]
  fn get_data_verifies_by_default() {
    let bundle = bundle_with_seed(0);
    let html = bundle.get_data("/index.html").unwrap();
    assert_eq!(html.unwrap(), INDEX_HTML.as_bytes());
  }

  #[test]
  fn get_data_verifies_checksum_with_seed() {
    let bundle = bundle_with_seed(42);
    let html = bundle
      .get_data_with_options("/index.html", verifying(42))
      .unwrap();
    assert_eq!(html.unwrap(), INDEX_HTML.as_bytes());

    // Reading with the wrong seed recomputes a different checksum.
    let err = bundle
      .get_data_with_options("/index.html", verifying(0))
      .unwrap_err();
    assert!(matches!(err, crate::Error::ChecksumMismatch));
  }

  #[test]
  fn get_data_detects_corrupted_entry() {
    let mut bundle = bundle_with_seed(0);
    // Corrupt a byte of the compressed payload, leaving its stored checksum intact.
    bundle.data[4] ^= 0xff;

    let err = bundle
      .get_data_with_options("/index.html", verifying(0))
      .unwrap_err();
    assert!(matches!(err, crate::Error::ChecksumMismatch));

    // Without verification the corruption is not reported as a checksum mismatch: it
    // either decompresses to garbage or fails inside lz4. This is why the checksum is
    // compared before decompression.
    let unverified =
      DataReadOptions::default().checksum(ChecksumReadOptions::default().verify(false));
    assert!(!matches!(
      bundle.get_data_with_options("/index.html", unverified),
      Err(crate::Error::ChecksumMismatch)
    ));
  }

  #[test]
  fn get_data_detects_corrupted_checksum() {
    let mut bundle = bundle_with_seed(0);
    let offset = checksum_offset(&bundle, "/index.html");
    bundle.data[offset] ^= 0xff;

    let err = bundle
      .get_data_with_options("/index.html", verifying(0))
      .unwrap_err();
    assert!(matches!(err, crate::Error::ChecksumMismatch));
  }

  #[cfg(feature = "async")]
  #[tokio::test]
  async fn async_get_data_verifies_checksum() {
    let bundle = bundle_with_seed(0);
    let mut raw = vec![];
    BundleWriter::new(Cursor::new(&mut raw))
      .write(&bundle)
      .unwrap();

    let mut reader = BundleReader::new(Cursor::new(&raw));
    let descriptor: BundleDescriptor = reader.read().unwrap();
    let options = verifying(0);

    let html = descriptor
      .async_get_data_with_options(Cursor::new(&raw), "/index.html", options)
      .await
      .unwrap();
    assert_eq!(html.unwrap(), INDEX_HTML.as_bytes());

    // Corrupting the on-disk payload is caught on read.
    let data_offset = descriptor.header().index_end_offset() as usize;
    let mut corrupted = raw.clone();
    corrupted[data_offset + 4] ^= 0xff;
    let err = descriptor
      .async_get_data_with_options(Cursor::new(&corrupted), "/index.html", options)
      .await
      .unwrap_err();
    assert!(matches!(err, crate::Error::ChecksumMismatch));
  }

  #[cfg(feature = "async")]
  #[tokio::test]
  async fn async_get_data() {
    let mut builder = Bundle::builder();
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/html".parse().unwrap());
    builder.insert_entry(
      "/index.html",
      BundleEntry::new(INDEX_HTML.as_bytes(), "text/html", Some(headers)),
    );
    builder.insert_entry(
      "/index.js",
      BundleEntry::new(INDEX_JS.as_bytes(), "text/javascript", None),
    );
    let bundle = builder.build().unwrap();
    let mut data = vec![];
    let mut writer = BundleWriter::new(Cursor::new(&mut data));
    writer.write(&bundle).unwrap();
    let mut reader = BundleReader::new(Cursor::new(&data));
    let descriptor: BundleDescriptor = reader.read().unwrap();
    let html = descriptor
      .async_get_data(Cursor::new(&data), "/index.html")
      .await
      .unwrap();
    assert_eq!(html.unwrap(), INDEX_HTML.as_bytes());
  }
}
