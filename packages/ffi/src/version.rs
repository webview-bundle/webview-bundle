/// Bundle format version. Currently only `V1` is defined.
#[derive(uniffi::Enum, Clone, Debug)]
pub enum Version {
  V1,
}

impl From<Version> for wvb::Version {
  fn from(value: Version) -> Self {
    match value {
      Version::V1 => wvb::Version::V1,
    }
  }
}

impl From<wvb::Version> for Version {
  fn from(value: wvb::Version) -> Self {
    match value {
      wvb::Version::V1 => Version::V1,
    }
  }
}
