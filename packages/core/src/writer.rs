pub trait Writer<T> {
  fn write(&mut self, data: &T) -> crate::Result<usize>;
}

#[cfg(feature = "async")]
pub trait AsyncWriter<T> {
  fn write(&mut self, data: &T) -> impl Future<Output = crate::Result<usize>>;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ChecksumWriteOptions {
  pub seed: u32,
}

impl ChecksumWriteOptions {
  pub fn seed(mut self, seed: u32) -> Self {
    self.seed = seed;
    self
  }
}
