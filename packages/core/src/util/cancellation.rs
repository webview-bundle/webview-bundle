use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default)]
pub struct Cancellation(CancellationToken);

impl From<CancellationToken> for Cancellation {
  fn from(token: CancellationToken) -> Self {
    Self(token)
  }
}

impl Cancellation {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn cancel(&self) {
    self.0.cancel();
  }

  pub fn is_cancelled(&self) -> bool {
    self.0.is_cancelled()
  }

  pub(crate) async fn cancelled(&self) {
    self.0.cancelled().await
  }

  pub(crate) async fn run_until_cancelled<F>(&self, fut: F) -> crate::Result<F::Output>
  where
    F: Future,
  {
    self
      .0
      .run_until_cancelled(fut)
      .await
      .ok_or(crate::Error::Cancelled)
  }
}
