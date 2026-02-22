use crate::builder::BundleBuilder;
use crate::checksum::{CHECKSUM_LEN, parse_checksum};
use crate::header::{Header, HeaderReader, HeaderWriter};
use crate::index::{Index, IndexEntry, IndexReader, IndexWriter};
use crate::reader::Reader;
use crate::writer::Writer;
use lz4_flex::decompress_size_prepended;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};

#[cfg(feature = "async")]
use crate::{
  AsyncHeaderReader, AsyncHeaderWriter, AsyncIndexReader, AsyncIndexWriter, AsyncReader,
  AsyncWriter,
};
#[cfg(feature = "async")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt};

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
/// let descriptor: BundleDescriptor = AsyncBundleReader::new(&mut file).read().await.unwrap();
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

  /// Reads file data from the bundle using the provided reader.
  ///
  /// Returns `None` if the path doesn't exist in the bundle.
  ///
  /// # Arguments
  ///
  /// * `reader` - A reader positioned at the start of the bundle file
  /// * `path` - File path in the bundle (e.g., "/index.html")
  pub fn get_data<R: Read + Seek>(&self, reader: R, path: &str) -> crate::Result<Option<Vec<u8>>> {
    if !self.index.contains_path(path) {
      return Ok(None);
    }
    let entry = self.index.get_entry(path).unwrap();
    let mut reader = BundleDataReader::new(reader, self.header.index_end_offset());
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

  /// Asynchronously reads file data from the bundle.
  ///
  /// Returns `None` if the path doesn't exist in the bundle.
  #[cfg(feature = "async")]
  pub async fn async_get_data<R: AsyncRead + AsyncSeek + Unpin>(
    &self,
    reader: R,
    path: &str,
  ) -> crate::Result<Option<Vec<u8>>> {
    if !self.index.contains_path(path) {
      return Ok(None);
    }
    let entry = self.index.get_entry(path).unwrap();
    let mut reader = AsyncBundleDataReader::new(reader, self.header.index_end_offset());
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
/// let bundle: Bundle = AsyncBundleReader::new(&mut file).read().await.unwrap();
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
  /// use wvb::Bundle;
  ///
  /// let mut builder = Bundle::builder();
  /// builder.add_file("/index.html", b"<html></html>", None);
  /// let bundle = builder.build();
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
  /// # use wvb::Bundle;
  /// let bundle = Bundle::builder()
  ///     .add_file("/test.txt", b"hello", None)
  ///     .build();
  ///
  /// let data = bundle.get_data("/test.txt").unwrap().unwrap();
  /// assert_eq!(data, b"hello");
  /// ```
  pub fn get_data(&self, path: &str) -> crate::Result<Option<Vec<u8>>> {
    if !self.descriptor.index.contains_path(path) {
      return Ok(None);
    }
    let entry = self.descriptor.index.get_entry(path).unwrap();
    let mut reader = BundleDataReader::new(Cursor::new(&self.data), 0);
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

fn read_entry(entry: &IndexEntry) -> (u64, Vec<u8>) {
  (entry.offset(), vec![0u8; entry.len() as usize])
}

fn parse_entry(buf: &[u8]) -> crate::Result<Vec<u8>> {
  let decompressed = decompress_size_prepended(buf)?;
  Ok(decompressed)
}

fn read_entry_checksum(entry: &IndexEntry) -> (u64, [u8; CHECKSUM_LEN]) {
  (entry.offset() + entry.len(), [0u8; CHECKSUM_LEN])
}

pub(crate) struct BundleDataReader<R: Read + Seek> {
  r: R,
  base_offset: u64,
}

impl<R: Read + Seek> BundleDataReader<R> {
  pub fn new(r: R, base_offset: u64) -> Self {
    Self { r, base_offset }
  }

  pub fn read_entry_data(&mut self, entry: &IndexEntry) -> crate::Result<Vec<u8>> {
    let (offset, mut buf) = read_entry(entry);
    self.r.seek(SeekFrom::Start(self.base_offset + offset))?;
    self.r.read_exact(&mut buf)?;
    parse_entry(&buf)
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
}

#[cfg(feature = "async")]
impl<R: AsyncRead + AsyncSeek + Unpin> AsyncBundleDataReader<R> {
  pub fn new(r: R, base_offset: u64) -> Self {
    Self { r, base_offset }
  }

  pub async fn read_entry_data(&mut self, entry: &IndexEntry) -> crate::Result<Vec<u8>> {
    let (offset, mut buf) = read_entry(entry);
    self
      .r
      .seek(SeekFrom::Start(self.base_offset + offset))
      .await?;
    self.r.read_exact(&mut buf).await?;
    parse_entry(&buf)
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
}

impl<R: Read + Seek> BundleReader<R> {
  pub fn new(r: R) -> Self {
    Self { r }
  }

  pub fn read_header(&mut self) -> crate::Result<Header> {
    let mut reader = HeaderReader::new(&mut self.r);
    let header = reader.read()?;
    Ok(header)
  }

  pub fn read_index(&mut self, header: Header) -> crate::Result<Index> {
    let mut reader = IndexReader::new(&mut self.r, header);
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
}

#[cfg(feature = "async")]
impl<R: AsyncRead + AsyncSeek + Unpin> AsyncBundleReader<R> {
  pub fn new(r: R) -> Self {
    Self { r }
  }

  pub async fn read_header(&mut self) -> crate::Result<Header> {
    let mut reader = AsyncHeaderReader::new(&mut self.r);
    let header = reader.read().await?;
    Ok(header)
  }

  pub async fn read_index(&mut self, header: Header) -> crate::Result<Index> {
    let mut reader = AsyncIndexReader::new(&mut self.r, header);
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
  use crate::BundleEntry;
  use crate::version::Version;
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
