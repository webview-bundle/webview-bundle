use napi_derive::napi;

#[napi]
pub const EXTENSION: &str = wvb::EXTENSION;
#[napi]
pub const MIME_TYPE: &str = wvb::MIME_TYPE;
#[napi]
pub const RUNTIME_VERSION: u8 = wvb::RUNTIME_VERSION;
#[napi]
pub const UPDATE_PROTOCOL_VERSION: &str = wvb::remote::UPDATE_PROTOCOL_VERSION;
