mod bundle;
mod error;
mod integrity;
mod protocol;
mod remote;
mod result;
mod signature;
mod source;
mod updater;

#[cfg(test)]
mod bindings;

use std::ffi::{CStr, c_char};
use std::sync::OnceLock;
use tokio::runtime::Runtime;

pub(crate) fn runtime() -> &'static Runtime {
  static RT: OnceLock<Runtime> = OnceLock::new();
  RT.get_or_init(|| {
    tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .build()
      .expect("failed to build tokio runtime")
  })
}

/// # Safety
/// `ptr` must be null or a valid NUL-terminated C string.
pub(crate) unsafe fn cstr(ptr: *const c_char) -> String {
  if ptr.is_null() {
    return String::new();
  }
  unsafe { CStr::from_ptr(ptr) }
    .to_string_lossy()
    .into_owned()
}

/// Copy `len` bytes the caller only guarantees for the duration of this call.
///
/// # Safety
/// `ptr` must be null, or point to `len` readable bytes.
pub(crate) unsafe fn owned_bytes(ptr: *const u8, len: usize) -> Vec<u8> {
  if ptr.is_null() || len == 0 {
    Vec::new()
  } else {
    unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec()
  }
}
