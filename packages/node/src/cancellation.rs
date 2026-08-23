use napi_derive::napi;
use wvb::util;

#[napi]
pub struct Cancellation {
  pub(crate) inner: util::cancellation::Cancellation,
}

#[napi]
impl Cancellation {
  #[allow(clippy::new_without_default)]
  #[napi(constructor)]
  pub fn new() -> Self {
    let inner = util::cancellation::Cancellation::new();
    Self { inner }
  }

  #[napi]
  pub fn cancel(&self) {
    self.inner.cancel();
  }

  #[napi]
  pub fn is_cancelled(&self) -> bool {
    self.inner.is_cancelled()
  }
}
