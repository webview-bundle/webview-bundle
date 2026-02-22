use crate::checksum::{CHECKSUM_LEN, make_checksum, parse_checksum, write_checksum};
use crate::reader::Reader;
use crate::version::{VERSION_LEN, Version};
use crate::writer::Writer;
use std::io::{Read, Seek, SeekFrom, Write};

#[cfg(feature = "async")]
use crate::reader::AsyncReader;
#[cfg(feature = "async")]
use crate::writer::AsyncWriter;
#[cfg(feature = "async")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt};

/// Bundle header containing format metadata.
///
/// The header is the first 17 bytes of a `.wvb` file:
///
/// | Magic (8) | Version (1) | Index Size (4) | Checksum (4) |
/// |-----------|-------------|----------------|--------------|
///
/// - **Magic Number**: `0xf09f8c90f09f8e81` (🌐🎁 in UTF-8)
/// - **Version**: Bundle format version (currently 0x01)
/// - **Index Size**: Size of the index section in bytes (u32, big-endian)
/// - **Checksum**: xxHash-32 checksum of the header data
///
/// # Example
///
/// ```
/// use wvb::{Header, Version};
///
/// let header = Header::new(Version::V1, 1024);
/// assert_eq!(header.version(), Version::V1);
/// assert_eq!(header.index_size(), 1024);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Header {
  version: Version,
  index_size: u32,
}

impl Header {
  /// Length of the magic number in bytes (8 bytes for "🌐🎁")
  pub const MAGIC_LEN: usize = 8;

  /// Magic number bytes: 0xf09f8c90f09f8e81 ("🌐🎁")
  pub const MAGIC: [u8; Header::MAGIC_LEN] = [0xf0, 0x9f, 0x8c, 0x90, 0xf0, 0x9f, 0x8e, 0x81];

  /// Offset of the magic number in the file
  pub const MAGIC_OFFSET: u64 = 0;

  /// Offset of the version byte
  pub const VERSION_OFFSET: u64 = Header::MAGIC_LEN as u64;

  /// Offset of the index size field
  pub const INDEX_SIZE_OFFSET: u64 = Self::VERSION_OFFSET + VERSION_LEN as u64;

  /// Length of the index size field in bytes
  pub const INDEX_SIZE_BYTES_LEN: usize = 4;

  /// Offset of the header checksum
  pub const CHECKSUM_OFFSET: u64 = Self::INDEX_SIZE_OFFSET + Self::INDEX_SIZE_BYTES_LEN as u64;

  /// Total size of the header in bytes (17 bytes)
  pub const END_OFFSET: u64 = Self::CHECKSUM_OFFSET + CHECKSUM_LEN as u64;

  /// Calculates the byte offset where the index section ends.
  ///
  /// This is the starting point of the data section.
  pub fn index_end_offset(&self) -> u64 {
    Self::END_OFFSET + self.index_size as u64 + CHECKSUM_LEN as u64
  }

  /// Creates a new header.
  ///
  /// # Arguments
  ///
  /// * `version` - Bundle format version
  /// * `index_size` - Size of the index section in bytes
  pub fn new(version: Version, index_size: u32) -> Self {
    Self {
      version,
      index_size,
    }
  }

  /// Returns the bundle format version.
  pub fn version(&self) -> Version {
    self.version
  }

  /// Returns the size of the index section in bytes.
  pub fn index_size(&self) -> u32 {
    self.index_size
  }
}

fn write_magic() -> Vec<u8> {
  Header::MAGIC.to_vec()
}

fn write_version(version: Version) -> Vec<u8> {
  version.bytes().to_vec()
}

fn write_index_size(index_size: u32) -> Vec<u8> {
  index_size.to_be_bytes().to_vec()
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct HeaderWriterOptions {
  pub(crate) checksum_seed: u32,
}

impl HeaderWriterOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn checksum_seed(&mut self, seed: u32) -> &mut Self {
    self.checksum_seed = seed;
    self
  }
}

pub struct HeaderWriter<W: Write> {
  w: W,
  options: HeaderWriterOptions,
}

impl<W: Write> HeaderWriter<W> {
  pub fn new(w: W) -> Self {
    Self {
      w,
      options: Default::default(),
    }
  }

  pub fn new_with_options(w: W, options: HeaderWriterOptions) -> Self {
    Self { w, options }
  }

  pub fn set_options(&mut self, options: HeaderWriterOptions) -> &mut Self {
    self.options = options;
    self
  }

  pub fn write_magic(&mut self) -> crate::Result<Vec<u8>> {
    let bytes = write_magic();
    self.w.write_all(&bytes)?;
    Ok(bytes)
  }

  pub fn write_version(&mut self, version: Version) -> crate::Result<Vec<u8>> {
    let bytes = write_version(version);
    self.w.write_all(&bytes)?;
    Ok(bytes)
  }

  pub fn write_index_size(&mut self, index_size: u32) -> crate::Result<Vec<u8>> {
    let bytes = write_index_size(index_size);
    self.w.write_all(&bytes)?;
    Ok(bytes)
  }

  pub fn write_checksum(&mut self, checksum: u32) -> crate::Result<Vec<u8>> {
    let bytes = write_checksum(checksum);
    self.w.write_all(&bytes)?;
    Ok(bytes)
  }
}

impl<W: Write> Writer<Header> for HeaderWriter<W> {
  fn write(&mut self, header: &Header) -> crate::Result<usize> {
    let mut bytes = vec![];
    bytes.extend(self.write_magic()?);
    bytes.extend(self.write_version(header.version)?);
    bytes.extend(self.write_index_size(header.index_size)?);

    let checksum = make_checksum(self.options.checksum_seed, &bytes);
    bytes.extend(self.write_checksum(checksum)?);
    Ok(bytes.len())
  }
}

#[cfg(feature = "async")]
pub struct AsyncHeaderWriter<W: AsyncWrite + Unpin> {
  w: W,
  options: HeaderWriterOptions,
}

#[cfg(feature = "async")]
impl<W: AsyncWrite + Unpin> AsyncHeaderWriter<W> {
  pub fn new(w: W) -> Self {
    Self {
      w,
      options: Default::default(),
    }
  }

  pub fn new_with_options(w: W, options: HeaderWriterOptions) -> Self {
    Self { w, options }
  }

  pub fn set_options(&mut self, options: HeaderWriterOptions) -> &mut Self {
    self.options = options;
    self
  }

  pub async fn write_magic(&mut self) -> crate::Result<Vec<u8>> {
    let bytes = write_magic();
    self.w.write_all(&bytes).await?;
    Ok(bytes)
  }

  pub async fn write_version(&mut self, version: Version) -> crate::Result<Vec<u8>> {
    let bytes = write_version(version);
    self.w.write_all(&bytes).await?;
    Ok(bytes)
  }

  pub async fn write_index_size(&mut self, index_size: u32) -> crate::Result<Vec<u8>> {
    let bytes = write_index_size(index_size);
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
impl<W: AsyncWrite + Unpin> AsyncWriter<Header> for AsyncHeaderWriter<W> {
  async fn write(&mut self, header: &Header) -> crate::Result<usize> {
    let mut bytes = vec![];
    bytes.extend(self.write_magic().await?);
    bytes.extend(self.write_version(header.version).await?);
    bytes.extend(self.write_index_size(header.index_size).await?);

    let checksum = make_checksum(self.options.checksum_seed, &bytes);
    bytes.extend(self.write_checksum(checksum).await?);
    Ok(bytes.len())
  }
}

fn read_magic() -> (u64, [u8; Header::MAGIC_LEN]) {
  (Header::MAGIC_OFFSET, [0u8; Header::MAGIC_LEN])
}

fn parse_magic(buf: &[u8; Header::MAGIC_LEN]) -> crate::Result<()> {
  if buf != Header::MAGIC.as_ref() {
    return Err(crate::Error::InvalidMagicNum);
  }
  Ok(())
}

fn read_version() -> (u64, [u8; VERSION_LEN]) {
  (Header::VERSION_OFFSET, [0u8; VERSION_LEN])
}

fn parse_version(buf: &[u8; VERSION_LEN]) -> crate::Result<Version> {
  if buf == Version::V1.bytes().as_ref() {
    return Ok(Version::V1);
  }
  Err(crate::Error::InvalidVersion)
}

fn read_index_size() -> (u64, [u8; Header::INDEX_SIZE_BYTES_LEN]) {
  (
    Header::INDEX_SIZE_OFFSET,
    [0u8; Header::INDEX_SIZE_BYTES_LEN],
  )
}

fn parse_index_size(buf: &[u8; Header::INDEX_SIZE_BYTES_LEN]) -> u32 {
  u32::from_be_bytes(AsRef::<[u8]>::as_ref(&buf).try_into().unwrap())
}

fn read_checksum() -> (u64, [u8; CHECKSUM_LEN]) {
  (Header::CHECKSUM_OFFSET, [0u8; CHECKSUM_LEN])
}

fn read_total() -> (u64, [u8; Header::CHECKSUM_OFFSET as usize]) {
  (
    Header::MAGIC_OFFSET,
    [0u8; Header::CHECKSUM_OFFSET as usize],
  )
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct HeaderReaderOptions {
  pub checksum_seed: u32,
  pub verify_checksum: bool,
}

impl HeaderReaderOptions {
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

pub struct HeaderReader<R: Read + Seek> {
  r: R,
  options: HeaderReaderOptions,
}

impl<R: Read + Seek> HeaderReader<R> {
  pub fn new(r: R) -> Self {
    Self::new_with_options(r, Default::default())
  }

  pub fn new_with_options(r: R, options: HeaderReaderOptions) -> Self {
    Self { r, options }
  }

  pub fn set_options(&mut self, options: HeaderReaderOptions) -> &mut Self {
    self.options = options;
    self
  }

  pub fn read_magic(&mut self) -> crate::Result<[u8; Header::MAGIC_LEN]> {
    let (offset, mut buf) = read_magic();
    self.r.seek(SeekFrom::Start(offset))?;
    self.r.read_exact(&mut buf)?;
    parse_magic(&buf)?;
    Ok(buf)
  }

  pub fn read_version(&mut self) -> crate::Result<Version> {
    let (offset, mut buf) = read_version();
    self.r.seek(SeekFrom::Start(offset))?;
    self.r.read_exact(&mut buf)?;
    parse_version(&buf)
  }

  pub fn read_index_size(&mut self) -> crate::Result<u32> {
    let (offset, mut buf) = read_index_size();
    self.r.seek(SeekFrom::Start(offset))?;
    self.r.read_exact(&mut buf)?;
    Ok(parse_index_size(&buf))
  }

  pub fn read_checksum(&mut self) -> crate::Result<u32> {
    let (offset, mut buf) = read_checksum();
    self.r.seek(SeekFrom::Start(offset))?;
    self.r.read_exact(&mut buf)?;
    let checksum = parse_checksum(&buf);
    Ok(checksum)
  }

  fn verify_checksum(&mut self, checksum: u32) -> crate::Result<()> {
    let (offset, mut total) = read_total();
    self.r.seek(SeekFrom::Start(offset))?;
    self.r.read_exact(&mut total)?;

    let expected_checksum = make_checksum(self.options.checksum_seed, &total);
    if checksum != expected_checksum {
      return Err(crate::Error::InvalidHeaderChecksum);
    }
    Ok(())
  }
}

impl<R: Read + Seek> Reader<Header> for HeaderReader<R> {
  fn read(&mut self) -> crate::Result<Header> {
    self.read_magic()?;
    let version = self.read_version()?;
    let index_size = self.read_index_size()?;
    let checksum = self.read_checksum()?;
    if self.options.verify_checksum {
      self.verify_checksum(checksum)?;
    }
    Ok(Header::new(version, index_size))
  }
}

#[cfg(feature = "async")]
pub struct AsyncHeaderReader<R: AsyncRead + AsyncSeek + Unpin> {
  r: R,
  options: HeaderReaderOptions,
}

#[cfg(feature = "async")]
impl<R: AsyncRead + AsyncSeek + Unpin> AsyncHeaderReader<R> {
  pub fn new(r: R) -> Self {
    Self::new_with_options(r, Default::default())
  }

  pub fn new_with_options(r: R, options: HeaderReaderOptions) -> Self {
    Self { r, options }
  }

  pub fn set_options(&mut self, options: HeaderReaderOptions) -> &mut Self {
    self.options = options;
    self
  }

  pub async fn read_magic(&mut self) -> crate::Result<[u8; Header::MAGIC_LEN]> {
    let (offset, mut buf) = read_magic();
    self.r.seek(SeekFrom::Start(offset)).await?;
    self.r.read_exact(&mut buf).await?;
    parse_magic(&buf)?;
    Ok(buf)
  }

  pub async fn read_version(&mut self) -> crate::Result<Version> {
    let (offset, mut buf) = read_version();
    self.r.seek(SeekFrom::Start(offset)).await?;
    self.r.read_exact(&mut buf).await?;
    parse_version(&buf)
  }

  pub async fn read_index_size(&mut self) -> crate::Result<u32> {
    let (offset, mut buf) = read_index_size();
    self.r.seek(SeekFrom::Start(offset)).await?;
    self.r.read_exact(&mut buf).await?;
    Ok(parse_index_size(&buf))
  }

  pub async fn read_checksum(&mut self) -> crate::Result<u32> {
    let (offset, mut buf) = read_checksum();
    self.r.seek(SeekFrom::Start(offset)).await?;
    self.r.read_exact(&mut buf).await?;
    let checksum = parse_checksum(&buf);
    Ok(checksum)
  }

  async fn verify_checksum(&mut self, checksum: u32) -> crate::Result<()> {
    let (offset, mut total) = read_total();
    self.r.seek(SeekFrom::Start(offset)).await?;
    self.r.read_exact(&mut total).await?;

    let expected_checksum = make_checksum(self.options.checksum_seed, &total);
    if checksum != expected_checksum {
      return Err(crate::Error::InvalidHeaderChecksum);
    }
    Ok(())
  }
}

#[cfg(feature = "async")]
impl<R: AsyncRead + AsyncSeek + Unpin> AsyncReader<Header> for AsyncHeaderReader<R> {
  async fn read(&mut self) -> crate::Result<Header> {
    self.read_magic().await?;
    let version = self.read_version().await?;
    let index_size = self.read_index_size().await?;
    let checksum = self.read_checksum().await?;
    if self.options.verify_checksum {
      self.verify_checksum(checksum).await?;
    }
    Ok(Header::new(version, index_size))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Cursor;

  #[test]
  fn read_and_write() {
    let header = Header::new(Version::V1, 1234);
    let mut buf = vec![];
    let mut writer = HeaderWriter::new(Cursor::new(&mut buf));
    writer.write(&header).unwrap();
    assert_eq!(
      buf,
      [
        240, 159, 140, 144, 240, 159, 142, 129, 1, 0, 0, 4, 210, 49, 56, 3, 16
      ]
    );
    let mut reader = HeaderReader::new(Cursor::new(&buf));
    let read_header = reader.read().unwrap();
    assert_eq!(header, read_header);
    assert_eq!(read_header.version(), Version::V1);
    assert_eq!(read_header.index_size(), 1234);
  }

  #[cfg(feature = "async")]
  #[tokio::test]
  async fn async_read_and_write() {
    let header = Header::new(Version::V1, 1234);
    let mut buf = vec![];
    let mut writer = AsyncHeaderWriter::new(Cursor::new(&mut buf));
    writer.write(&header).await.unwrap();
    assert_eq!(
      buf,
      [
        240, 159, 140, 144, 240, 159, 142, 129, 1, 0, 0, 4, 210, 49, 56, 3, 16
      ]
    );
    let mut reader = AsyncHeaderReader::new(Cursor::new(&buf));
    let read_header = reader.read().await.unwrap();
    assert_eq!(header, read_header);
    assert_eq!(read_header.version(), Version::V1);
    assert_eq!(read_header.index_size(), 1234);
  }
}
