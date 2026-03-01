mod bundle;
mod http_ext;
#[cfg(feature = "protocol-local")]
mod local;
mod uri;

use async_trait::async_trait;
use std::borrow::Cow;

pub type ProtocolResponse = http::Response<Cow<'static, [u8]>>;

#[async_trait]
pub trait Protocol: Send + Sync {
  async fn handle(&self, request: http::Request<Vec<u8>>) -> crate::Result<ProtocolResponse>;
}

pub use bundle::*;
#[cfg(feature = "protocol-local")]
pub use local::*;
