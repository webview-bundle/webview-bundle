#[derive(uniffi::Object, Clone, Debug)]
pub struct Cancellation {
  pub(crate) inner: wvb::util::cancellation::Cancellation,
}

#[uniffi::export]
impl Cancellation {
  #[uniffi::constructor]
  pub fn new() -> Self {
    Self {
      inner: wvb::util::cancellation::Cancellation::new(),
    }
  }

  pub fn cancel(&self) {
    self.inner.cancel();
  }

  pub fn is_cancelled(&self) -> bool {
    self.inner.is_cancelled()
  }
}
