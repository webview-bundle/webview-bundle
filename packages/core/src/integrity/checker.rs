use crate::integrity::Integrity;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;

pub type CustomChecker = dyn Fn(
    &[u8],
    &str,
  ) -> Pin<
    Box<
      dyn Future<Output = Result<bool, Box<dyn std::error::Error + Send + Sync + 'static>>>
        + Send
        + 'static,
    >,
  > + Send
  + Sync;

#[non_exhaustive]
#[derive(Default, Clone)]
pub enum IntegrityChecker {
  #[default]
  Default,
  Custom(Arc<CustomChecker>),
}

impl std::fmt::Debug for IntegrityChecker {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let name = match self {
      Self::Default => "Default",
      Self::Custom(_) => "Custom",
    };
    write!(f, "IntegrityChecker::{name}")
  }
}

impl IntegrityChecker {
  pub async fn check(&self, integrity: &str, data: &[u8]) -> crate::Result<()> {
    match self {
      Self::Default => {
        let integrity = Integrity::from_str(integrity)?;
        if !integrity.validate(data) {
          return Err(crate::Error::IntegrityVerifyFailed);
        }
        Ok(())
      }
      Self::Custom(checker) => {
        if !checker(data, integrity)
          .await
          .map_err(crate::Error::generic)?
        {
          return Err(crate::Error::IntegrityVerifyFailed);
        }
        Ok(())
      }
    }
  }
}
