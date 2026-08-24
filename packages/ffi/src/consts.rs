#[uniffi::export]
pub fn extension() -> String {
  wvb::EXTENSION.to_string()
}

#[uniffi::export]
pub fn mime_type() -> String {
  wvb::MIME_TYPE.to_string()
}

#[uniffi::export]
pub fn runtime_version() -> u8 {
  wvb::RUNTIME_VERSION
}

#[uniffi::export]
pub fn update_protocol_version() -> String {
  wvb::remote::UPDATE_PROTOCOL_VERSION.to_string()
}
