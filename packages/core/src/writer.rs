pub trait Writer<T> {
  fn write(&mut self, data: &T) -> crate::Result<usize>;
}

#[cfg(feature = "async")]
pub trait AsyncWriter<T> {
  fn write(&mut self, data: &T) -> impl Future<Output = crate::Result<usize>>;
}
