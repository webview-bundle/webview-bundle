use crate::checksum::{CHECKSUM_LEN, make_checksum, parse_checksum, write_checksum};
use crate::header::Header;
use crate::reader::Reader;
use crate::writer::Writer;
use bincode::de::Decoder;
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{Decode, Encode, config, decode_from_slice, encode_to_vec};
use http::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::{Deref, DerefMut};

#[cfg(feature = "async")]
use crate::reader::AsyncReader;
#[cfg(feature = "async")]
use crate::writer::AsyncWriter;
#[cfg(feature = "async")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt};

/// Metadata for a single file in the bundle.
///
/// An index entry contains:
/// - File location (offset and length in the data section)
/// - HTTP metadata (content-type, content-length)
/// - Optional HTTP headers for protocol serving
///
/// # Example
///
/// ```
/// use wvb::IndexEntry;
///
/// let entry = IndexEntry::new(0, 1024, "text/html", 512);
/// assert_eq!(entry.content_type(), "text/html");
/// assert_eq!(entry.offset(), 0);
/// assert_eq!(entry.len(), 1024);
/// ```
#[derive(Debug, PartialEq, Clone)]
pub struct IndexEntry {
  offset: u64,
  len: u64,
  content_type: String,
  content_length: u64,
  pub(crate) headers: HeaderMap,
}

impl IndexEntry {
  /// Creates a new index entry.
  ///
  /// # Arguments
  ///
  /// * `offset` - Byte offset in the data section (after compression)
  /// * `len` - Length of compressed data in bytes
  /// * `content_type` - MIME type of the file
  /// * `content_length` - Original file size before compression
  pub fn new(offset: u64, len: u64, content_type: impl Into<String>, content_length: u64) -> Self {
    Self {
      offset,
      len,
      content_type: content_type.into(),
      content_length,
      headers: HeaderMap::default(),
    }
  }

  /// Returns the MIME type of the file.
  pub fn content_type(&self) -> &str {
    &self.content_type
  }

  /// Returns the original file size before compression.
  pub fn content_length(&self) -> u64 {
    self.content_length
  }

  /// Returns HTTP headers associated with this file.
  pub fn headers(&self) -> &HeaderMap {
    &self.headers
  }

  /// Returns the byte offset in the data section.
  pub fn offset(&self) -> u64 {
    self.offset
  }

  /// Returns `true` if the compressed data length is zero.
  pub fn is_empty(&self) -> bool {
    self.len == 0
  }

  /// Returns the length of the compressed data in bytes.
  pub fn len(&self) -> u64 {
    self.len
  }
}

impl Encode for IndexEntry {
  fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
    let mut headers: Vec<(String, Vec<u8>)> = Vec::with_capacity(self.headers.len());
    for (name, value) in self.headers.iter() {
      headers.push((name.as_str().to_string(), value.as_bytes().to_vec()));
    }
    let tuple = (
      self.offset,
      self.len,
      self.content_type.as_bytes(),
      self.content_length,
      headers,
    );
    tuple.encode(encoder)?;
    Ok(())
  }
}

impl<T> Decode<T> for IndexEntry {
  fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
    let (offset, len, content_type_raw, content_length, pairs): (
      u64,
      u64,
      Vec<u8>,
      u64,
      Vec<(String, Vec<u8>)>,
    ) = Decode::decode(decoder)?;
    let content_type = String::from_utf8(content_type_raw)
      .map_err(|_| DecodeError::OtherString("invalid content type".into()))?;
    let mut headers = HeaderMap::new();
    for (name, value_bytes) in pairs {
      let header_name = HeaderName::try_from(name.as_str())
        .map_err(|_| DecodeError::OtherString("invalid header name".into()))?;
      let header_value = HeaderValue::from_bytes(&value_bytes)
        .map_err(|_| DecodeError::OtherString("invalid header value".into()))?;
      headers.append(header_name, header_value);
    }
    Ok(IndexEntry {
      offset,
      len,
      content_type,
      content_length,
      headers,
    })
  }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub(crate) struct IndexEntryMap(pub(crate) HashMap<String, IndexEntry>);

/// Bundle index mapping file paths to their metadata.
///
/// The index is a HashMap stored as binary-encoded data in the bundle file,
/// immediately after the header. It maps file paths (keys) to `IndexEntry`
/// metadata (values).
///
/// The index section includes an xxHash-32 checksum for verification.
///
/// # Example
///
/// ```
/// use wvb::{Bundle, BundleBuilder};
///
/// let bundle = Bundle::builder()
///     .add_file("/index.html", b"<html></html>", None)
///     .add_file("/app.js", b"console.log('hello');", None)
///     .build();
///
/// let index = bundle.descriptor().index();
/// assert!(index.contains_path("/index.html"));
/// assert!(index.contains_path("/app.js"));
/// assert_eq!(index.len(), 2);
/// ```
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Index {
  entries: IndexEntryMap,
}

impl Index {
  /// Creates a new empty index.
  pub fn new() -> Self {
    Self::default()
  }

  /// Creates a new index with pre-allocated capacity.
  ///
  /// Use this when you know how many files will be in the bundle.
  pub fn new_with_capacity(capacity: usize) -> Self {
    Self {
      entries: IndexEntryMap(HashMap::with_capacity(capacity)),
    }
  }
}

impl Encode for IndexEntryMap {
  fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
    self.0.encode(encoder)
  }
}

impl<T> Decode<T> for IndexEntryMap {
  fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
    let map = HashMap::<String, IndexEntry>::decode(decoder)?;
    Ok(IndexEntryMap(map))
  }
}

impl Deref for IndexEntryMap {
  type Target = HashMap<String, IndexEntry>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for IndexEntryMap {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl Index {
  pub fn entries(&self) -> &HashMap<String, IndexEntry> {
    &self.entries
  }

  pub fn insert_entry<S: Into<String>>(
    &mut self,
    path: S,
    entry: IndexEntry,
  ) -> Option<IndexEntry> {
    self.entries.insert(path.into(), entry)
  }

  pub fn get_entry(&self, path: &str) -> Option<&IndexEntry> {
    self.entries.get(path)
  }

  pub fn get_entry_mut(&mut self, path: &str) -> Option<&mut IndexEntry> {
    self.entries.get_mut(path)
  }

  pub fn remove_entry(&mut self, path: &str) -> Option<IndexEntry> {
    self.entries.remove(path)
  }

  pub fn contains_path(&self, path: &str) -> bool {
    self.entries.contains_key(path)
  }
}

fn write_index(index: &Index) -> crate::Result<Vec<u8>> {
  let config = config::standard().with_big_endian();
  let bytes = encode_to_vec(&index.entries, config).map_err(|e| crate::Error::Encode {
    error: e,
    message: "fail to encode index".to_string(),
  })?;
  Ok(bytes)
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct IndexWriterOptions {
  pub(crate) checksum_seed: u32,
}

impl IndexWriterOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn checksum_seed(&mut self, seed: u32) -> &mut Self {
    self.checksum_seed = seed;
    self
  }
}

pub struct IndexWriter<W: Write> {
  w: W,
  options: IndexWriterOptions,
}

impl<W: Write> IndexWriter<W> {
  pub fn new(w: W) -> Self {
    Self {
      w,
      options: Default::default(),
    }
  }

  pub fn new_with_options(w: W, options: IndexWriterOptions) -> Self {
    Self { w, options }
  }

  pub fn write_index(&mut self, index: &Index) -> crate::Result<Vec<u8>> {
    let bytes = write_index(index)?;
    self.w.write_all(&bytes)?;
    Ok(bytes)
  }

  pub fn write_checksum(&mut self, checksum: u32) -> crate::Result<Vec<u8>> {
    let bytes = write_checksum(checksum);
    self.w.write_all(&bytes)?;
    Ok(bytes)
  }
}

impl<W: Write> Writer<Index> for IndexWriter<W> {
  fn write(&mut self, index: &Index) -> crate::Result<usize> {
    let mut bytes = vec![];
    bytes.extend(self.write_index(index)?);
    let checksum = make_checksum(self.options.checksum_seed, &bytes);
    bytes.extend(self.write_checksum(checksum)?);
    Ok(bytes.len())
  }
}

#[cfg(feature = "async")]
pub struct AsyncIndexWriter<W: AsyncWrite + Unpin> {
  w: W,
  options: IndexWriterOptions,
}

#[cfg(feature = "async")]
impl<W: AsyncWrite + Unpin> AsyncIndexWriter<W> {
  pub fn new(w: W) -> Self {
    Self {
      w,
      options: Default::default(),
    }
  }

  pub fn new_with_options(w: W, options: IndexWriterOptions) -> Self {
    Self { w, options }
  }

  pub async fn write_index(&mut self, index: &Index) -> crate::Result<Vec<u8>> {
    let bytes = write_index(index)?;
    self.w.write_all(&bytes).await?;
    Ok(bytes)
  }

  pub async fn write_checksum(&mut self, checksum: u32) -> crate::Result<Vec<u8>> {
    let bytes = write_checksum(checksum);
    self.w.write_all(&bytes).await?;
    Ok(bytes)
  }
}

#[cfg(feature = "async")]
impl<W: AsyncWrite + Unpin> AsyncWriter<Index> for AsyncIndexWriter<W> {
  async fn write(&mut self, index: &Index) -> crate::Result<usize> {
    let mut bytes = vec![];
    bytes.extend(self.write_index(index).await?);
    let checksum = make_checksum(self.options.checksum_seed, &bytes);
    bytes.extend(self.write_checksum(checksum).await?);
    Ok(bytes.len())
  }
}

fn read_index(header: &Header) -> (u64, Vec<u8>) {
  (Header::END_OFFSET, vec![0u8; header.index_size() as usize])
}

fn parse_index(buf: &[u8]) -> crate::Result<Index> {
  let config = config::standard().with_big_endian();
  let (entries, _): (IndexEntryMap, _) =
    decode_from_slice(buf, config).map_err(|e| crate::Error::Decode {
      error: e,
      message: "fail to decode index".to_string(),
    })?;
  Ok(Index { entries })
}

fn read_checksum(header: &Header) -> (u64, [u8; CHECKSUM_LEN]) {
  (
    Header::END_OFFSET + header.index_size() as u64,
    [0u8; CHECKSUM_LEN],
  )
}

fn read_total(header: &Header) -> (u64, Vec<u8>) {
  (Header::END_OFFSET, vec![0u8; header.index_size() as usize])
}

pub struct IndexReader<R: Read + Seek> {
  r: R,
  header: Header,
  options: IndexReaderOptions,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct IndexReaderOptions {
  pub checksum_seed: u32,
  pub verify_checksum: bool,
}

impl IndexReaderOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn checksum_seed(mut self, seed: u32) -> Self {
    self.checksum_seed = seed;
    self
  }

  pub fn verify_checksum(mut self, verify: bool) -> Self {
    self.verify_checksum = verify;
    self
  }
}

impl<R: Read + Seek> IndexReader<R> {
  pub fn new(r: R, header: Header) -> Self {
    Self::new_with_options(r, header, Default::default())
  }

  pub fn new_with_options(r: R, header: Header, options: IndexReaderOptions) -> Self {
    Self { r, header, options }
  }

  pub fn read_index(&mut self) -> crate::Result<Index> {
    let (offset, mut buf) = read_index(&self.header);
    self.r.seek(SeekFrom::Start(offset))?;
    self.r.read_exact(&mut buf)?;
    parse_index(&buf)
  }

  pub fn read_checksum(&mut self) -> crate::Result<u32> {
    let (offset, mut buf) = read_checksum(&self.header);
    self.r.seek(SeekFrom::Start(offset))?;
    self.r.read_exact(&mut buf)?;
    let checksum = parse_checksum(&buf);
    Ok(checksum)
  }

  fn verify_checksum(&mut self, checksum: u32) -> crate::Result<()> {
    let (offset, mut buf) = read_total(&self.header);
    self.r.seek(SeekFrom::Start(offset))?;
    self.r.read_exact(&mut buf)?;

    let expected_checksum = make_checksum(self.options.checksum_seed, &buf);
    if checksum != expected_checksum {
      return Err(crate::Error::InvalidIndexChecksum);
    }
    Ok(())
  }
}

impl<R: Read + Seek> Reader<Index> for IndexReader<R> {
  fn read(&mut self) -> crate::Result<Index> {
    let index = self.read_index()?;
    let checksum = self.read_checksum()?;
    if self.options.verify_checksum {
      self.verify_checksum(checksum)?;
    }
    Ok(index)
  }
}

#[cfg(feature = "async")]
pub struct AsyncIndexReader<R: AsyncRead + AsyncSeek + Unpin> {
  r: R,
  header: Header,
  options: IndexReaderOptions,
}

#[cfg(feature = "async")]
impl<R: AsyncRead + AsyncSeek + Unpin> AsyncIndexReader<R> {
  pub fn new(r: R, header: Header) -> Self {
    Self::new_with_options(r, header, Default::default())
  }

  pub fn new_with_options(r: R, header: Header, options: IndexReaderOptions) -> Self {
    Self { r, header, options }
  }

  pub async fn read_index(&mut self) -> crate::Result<Index> {
    let (offset, mut buf) = read_index(&self.header);
    self.r.seek(SeekFrom::Start(offset)).await?;
    self.r.read_exact(&mut buf).await?;
    parse_index(&buf)
  }

  pub async fn read_checksum(&mut self) -> crate::Result<u32> {
    let (offset, mut buf) = read_checksum(&self.header);
    self.r.seek(SeekFrom::Start(offset)).await?;
    self.r.read_exact(&mut buf).await?;
    let checksum = parse_checksum(&buf);
    Ok(checksum)
  }

  async fn verify_checksum(&mut self, checksum: u32) -> crate::Result<()> {
    let (offset, mut buf) = read_total(&self.header);
    self.r.seek(SeekFrom::Start(offset)).await?;
    self.r.read_exact(&mut buf).await?;

    let expected_checksum = make_checksum(self.options.checksum_seed, &buf);
    if checksum != expected_checksum {
      return Err(crate::Error::InvalidIndexChecksum);
    }
    Ok(())
  }
}

#[cfg(feature = "async")]
impl<R: AsyncRead + AsyncSeek + Unpin> AsyncReader<Index> for AsyncIndexReader<R> {
  async fn read(&mut self) -> crate::Result<Index> {
    let index = self.read_index().await?;
    let checksum = self.read_checksum().await?;
    if self.options.verify_checksum {
      self.verify_checksum(checksum).await?;
    }
    Ok(index)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn index() {
    let mut index = Index::default();
    let entry = IndexEntry::new(0, 0, "application/javascript", 42);
    index.insert_entry("/index.jsx", entry);

    assert!(index.contains_path("/index.jsx"));
    assert_eq!(
      index
        .get_entry("/index.jsx")
        .map(|x| x.content_type().to_string())
        .unwrap(),
      "application/javascript"
    );
    assert_eq!(
      index
        .get_entry("/index.jsx")
        .map(|x| x.content_length())
        .unwrap(),
      42
    );
  }

  #[cfg(feature = "async")]
  #[tokio::test]
  async fn async_read_and_write() {
    use crate::HeaderWriter;
    use crate::Version;
    use std::io::Cursor;

    let mut index = Index::default();
    let entry = IndexEntry::new(0, 0, "application/javascript", 55);
    index.insert_entry("/index.jsx", entry);

    let mut buf = vec![];
    let mut writer = AsyncIndexWriter::new(Cursor::new(&mut buf));
    writer.write(&index).await.unwrap();
    assert_eq!(
      buf,
      vec![
        1, 10, 47, 105, 110, 100, 101, 120, 46, 106, 115, 120, 0, 0, 22, 97, 112, 112, 108, 105,
        99, 97, 116, 105, 111, 110, 47, 106, 97, 118, 97, 115, 99, 114, 105, 112, 116, 55, 0, 198,
        6, 219, 80
      ]
    );
    let header = Header::new(Version::V1, (buf.len() - CHECKSUM_LEN) as u32);
    let mut total = vec![];
    HeaderWriter::new(Cursor::new(&mut total))
      .write(&header)
      .unwrap();
    total.extend(&buf);
    let mut reader = AsyncIndexReader::new(Cursor::new(&total), header);
    let read_index = reader.read().await.unwrap();
    assert_eq!(read_index, index);
  }
}
