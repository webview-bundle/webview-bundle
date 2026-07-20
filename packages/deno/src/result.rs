use crate::error::ErrorCode;
use std::ffi::{CString, c_char};

/// Result of a data-API call: `json` (payload on success / `{ code, message }` on error) +
/// optional `body` bytes.
pub(crate) struct WvbResult {
  ok: bool,
  json: CString,
  body: Vec<u8>,
}

pub(crate) fn ok_result(json: serde_json::Value, body: Vec<u8>) -> *mut WvbResult {
  let text = serde_json::to_string(&json).unwrap_or_else(|_| "null".to_string());
  Box::into_raw(Box::new(WvbResult {
    ok: true,
    json: CString::new(text).unwrap_or_default(),
    body,
  }))
}

/// An error result carrying the stable code alongside the message, so `@wvb/deno` can rebuild a
/// `WebviewBundleError` with the same code the other bindings use.
pub(crate) fn err_result(code: ErrorCode, message: String) -> *mut WvbResult {
  let json = serde_json::json!({ "code": code.as_str(), "message": message });
  let text = serde_json::to_string(&json).unwrap_or_else(|_| "null".to_string());
  Box::into_raw(Box::new(WvbResult {
    ok: false,
    json: CString::new(text).unwrap_or_default(),
    body: Vec::new(),
  }))
}

/// A `wvb` core error, tagged with its [`wvb::ErrorCode`] as the `core.<code>` wire code.
pub(crate) fn core_err(e: wvb::Error) -> *mut WvbResult {
  err_result(e.code().into(), e.to_string())
}

/// A handle argument was null, or had already been freed.
pub(crate) fn null_handle_err(what: &str) -> *mut WvbResult {
  err_result(ErrorCode::NullHandle, format!("{what} handle is null"))
}

/// Serialize a wire type to its JSON value. The wire types (see [`wire`]) are the single definition
/// of every DTO shape — carrying the `From<&core::T>` guards and the generated `lib/bindings.ts` —
/// so serializing them here is what keeps the runtime JSON and the TypeScript in lockstep.
pub(crate) fn wire_json<T: serde::Serialize>(value: T) -> serde_json::Value {
  serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

/// An ok result whose payload is the address of a freshly-boxed handle, as a decimal string. A
/// pointer can exceed a JS-safe integer, so it crosses as a string that `@wvb/deno` turns back into
/// a `Deno.PointerValue` via `Deno.UnsafePointer.create(BigInt(...))`. Errors still come back as the
/// usual `{ code, message }` result, so — unlike a bare null return — the reason is preserved.
pub(crate) fn ok_handle<T>(ptr: *mut T) -> *mut WvbResult {
  ok_result(
    serde_json::Value::String((ptr as usize).to_string()),
    Vec::new(),
  )
}

/// `Some(bytes)` → json `true` + the bytes as the body; `None` → json `null`, so a caller can tell
/// "present but empty" from "path absent" (mirroring `wvb_source_load_version`'s `Ok(None)`).
pub(crate) fn data_result(data: Option<Vec<u8>>) -> *mut WvbResult {
  match data {
    Some(bytes) => ok_result(serde_json::Value::Bool(true), bytes),
    None => ok_result(serde_json::Value::Null, Vec::new()),
  }
}

pub(crate) fn checksum_result(checksum: Option<u32>) -> *mut WvbResult {
  ok_result(
    checksum.map_or(serde_json::Value::Null, |c| serde_json::json!(c)),
    Vec::new(),
  )
}

/// # Safety
/// `result` must be null or a valid `WvbResult` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_result_ok(result: *const WvbResult) -> u8 {
  match unsafe { result.as_ref() } {
    Some(r) if r.ok => 1,
    _ => 0,
  }
}

/// Borrowed pointer to the result's JSON/message string (valid until `wvb_result_free`).
///
/// # Safety
/// `result` must be null or a valid `WvbResult` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_result_json(result: *const WvbResult) -> *const c_char {
  match unsafe { result.as_ref() } {
    Some(r) => r.json.as_ptr(),
    None => std::ptr::null(),
  }
}

/// # Safety
/// `result` must be null or a valid `WvbResult` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_result_body_ptr(result: *const WvbResult) -> *const u8 {
  match unsafe { result.as_ref() } {
    Some(r) => r.body.as_ptr(),
    None => std::ptr::null(),
  }
}

/// # Safety
/// `result` must be null or a valid `WvbResult` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_result_body_len(result: *const WvbResult) -> usize {
  unsafe { result.as_ref() }
    .map(|r| r.body.len())
    .unwrap_or(0)
}

/// # Safety
/// `result` must be null or a pointer previously returned by a Remote/Updater call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_result_free(result: *mut WvbResult) {
  if !result.is_null() {
    drop(unsafe { Box::from_raw(result) });
  }
}
