mod a;

pub use a::Header;
use std::sync::Arc;

#[derive(uniffi::Object)]
pub struct BundleProtocol {}

#[uniffi::export(async_runtime = "tokio")]
impl BundleProtocol {
  #[uniffi::constructor]
  pub fn new() -> Arc<Self> {
    Arc::new(Self {})
  }

  pub async fn handle(&self) -> String {
    "Hello World".to_string()
  }
}

uniffi::setup_scaffolding!();
