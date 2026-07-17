pub trait Reader<T> {
  fn read(&mut self) -> crate::Result<T>;
}

#[cfg(feature = "async")]
pub trait AsyncReader<T> {
  fn read(&mut self) -> impl Future<Output = crate::Result<T>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChecksumReadOptions {
  pub verify: bool,
  pub seed: u32,
}

impl Default for ChecksumReadOptions {
  fn default() -> Self {
    Self {
      verify: true,
      seed: 0,
    }
  }
}

impl ChecksumReadOptions {
  pub fn verify(mut self, verify: bool) -> Self {
    self.verify = verify;
    self
  }

  pub fn seed(mut self, seed: u32) -> Self {
    self.seed = seed;
    self
  }
}
