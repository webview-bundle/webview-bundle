#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use specta::Type;

/// How a bundle section's xxHash checksum is verified when that section is read. The same options
/// apply to the header, the index and each entry's data.
///
/// This detects corruption, not tampering: the seed is not secret, so whatever can rewrite the
/// bytes can rewrite the checksum.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChecksumReadOptions {
  /// Verify the section's checksum when it is read. Default: `true`.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub verify: Option<bool>,
  /// The seed the checksum was built with. Default: `0`.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub seed: Option<u32>,
}

impl From<ChecksumReadOptions> for wvb::ChecksumReadOptions {
  fn from(value: ChecksumReadOptions) -> Self {
    let mut options = Self::default();
    if let Some(verify) = value.verify {
      options = options.verify(verify);
    }
    if let Some(seed) = value.seed {
      options = options.seed(seed);
    }
    options
  }
}

/// The seed an xxHash checksum is written with.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChecksumWriteOptions {
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub seed: Option<u32>,
}

impl From<ChecksumWriteOptions> for wvb::ChecksumWriteOptions {
  fn from(value: ChecksumWriteOptions) -> Self {
    let mut options = Self::default();
    if let Some(seed) = value.seed {
      options = options.seed(seed);
    }
    options
  }
}

macro_rules! read_options {
  ($($(#[$attr:meta])* $name:ident => $core:path),+ $(,)?) => {
    $(
      $(#[$attr])*
      #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Type)]
      #[serde(rename_all = "camelCase", deny_unknown_fields)]
      pub struct $name {
        #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
        pub checksum: Option<ChecksumReadOptions>,
      }

      impl From<$name> for $core {
        fn from(value: $name) -> Self {
          let mut options = Self::default();
          if let Some(checksum) = value.checksum {
            options = options.checksum(checksum.into());
          }
          options
        }
      }
    )+
  };
}

read_options! {
  /// How a bundle's header is read.
  HeaderReadOptions => wvb::HeaderReadOptions,
  /// How a bundle's index is read.
  IndexReadOptions => wvb::IndexReadOptions,
  /// How each entry's data is read out of a bundle's data section.
  DataReadOptions => wvb::DataReadOptions,
}

macro_rules! write_options {
  ($($(#[$attr:meta])* $name:ident => $core:path),+ $(,)?) => {
    $(
      $(#[$attr])*
      #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Type)]
      #[serde(rename_all = "camelCase", deny_unknown_fields)]
      pub struct $name {
        #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
        pub checksum: Option<ChecksumWriteOptions>,
      }

      impl From<$name> for $core {
        fn from(value: $name) -> Self {
          let mut options = Self::default();
          if let Some(checksum) = value.checksum {
            options = options.checksum(checksum.into());
          }
          options
        }
      }
    )+
  };
}

write_options! {
  /// How a bundle's header section is written.
  HeaderWriterOptions => wvb::HeaderWriterOptions,
  /// How a bundle's index section is written.
  IndexWriterOptions => wvb::IndexWriterOptions,
}

/// Options for `BundleBuilder.build`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleBuilderOptions {
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub header: Option<HeaderWriterOptions>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub index: Option<IndexWriterOptions>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data_checksum: Option<ChecksumWriteOptions>,
}

impl From<BundleBuilderOptions> for wvb::BundleBuilderOptions {
  fn from(value: BundleBuilderOptions) -> Self {
    let mut options = Self::default();
    if let Some(header) = value.header {
      options = options.header(header.into());
    }
    if let Some(index) = value.index {
      options = options.index(index.into());
    }
    if let Some(data_checksum) = value.data_checksum {
      options = options.data_checksum(data_checksum.into());
    }
    options
  }
}
