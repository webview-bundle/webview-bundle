pub mod bundle;
mod error;
pub mod http;
pub mod integrity;
pub mod js;
pub mod mime;
pub mod protocol;
pub mod remote;
pub mod signature;
pub mod source;
pub mod updater;
pub mod version;

pub use error::{Error, Outcome};
pub type Result<T> = std::result::Result<T, Error>;
