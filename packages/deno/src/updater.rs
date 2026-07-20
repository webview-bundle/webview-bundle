use crate::integrity::parse_integrity_policy;
use crate::remote::{WvbRemote, list_info_json, remote_info_json};
use crate::result::{WvbResult, core_err, null_handle_err, ok_result, wire_json};
use crate::signature::build_signature_verifier;
use crate::source::WvbSource;
use crate::{cstr, runtime};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::ffi::c_char;
use wvb::updater::{
  self, Updater as CoreUpdater, UpdaterIntegrityOptions, UpdaterOptions, UpdaterSignatureOptions,
};

/// The result of an updater `getUpdate`: the available remote version plus the local one.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BundleUpdateInfo {
  pub name: String,
  pub version: String,
  #[specta(optional)]
  pub local_version: Option<String>,
  pub is_available: bool,
  #[specta(optional)]
  pub etag: Option<String>,
  #[specta(optional)]
  pub integrity: Option<String>,
  #[specta(optional)]
  pub signature: Option<String>,
  #[specta(optional)]
  pub last_modified: Option<String>,
}

impl From<&updater::BundleUpdateInfo> for BundleUpdateInfo {
  fn from(info: &updater::BundleUpdateInfo) -> Self {
    Self {
      name: info.name.clone(),
      version: info.version.clone(),
      local_version: info.local_version.clone(),
      is_available: info.is_available,
      etag: info.etag.clone(),
      integrity: info.integrity.clone(),
      signature: info.signature.clone(),
      last_modified: info.last_modified.clone(),
    }
  }
}

pub struct WvbUpdater {
  inner: CoreUpdater,
}

fn update_info_json(info: &wvb::updater::BundleUpdateInfo) -> serde_json::Value {
  wire_json(BundleUpdateInfo::from(info))
}

/// # Safety
/// `source`/`remote` must be valid handles; `options_json` null or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_new(
  source: *const WvbSource,
  remote: *const WvbRemote,
  options_json: *const c_char,
) -> *mut WvbUpdater {
  let (Some(source), Some(remote)) = (unsafe { source.as_ref() }, unsafe { remote.as_ref() })
  else {
    return std::ptr::null_mut();
  };
  let raw = unsafe { cstr(options_json) };
  let config = if raw.is_empty() {
    None
  } else {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
      return std::ptr::null_mut();
    };
    let mut config = UpdaterOptions::default();
    if let Some(channel) = value.get("channel").and_then(|x| x.as_str()) {
      config = config.channel(channel.to_string());
    }
    // An unknown policy value fails closed (null updater) rather than being silently ignored.
    match value.get("integrityPolicy") {
      None | Some(serde_json::Value::Null) => {}
      Some(x) => match x.as_str().and_then(parse_integrity_policy) {
        Some(policy) => {
          config = config.integrity(UpdaterIntegrityOptions::default().policy(policy))
        }
        None => return std::ptr::null_mut(),
      },
    }
    // A present-but-unbuildable signatureVerifier fails closed (null updater) rather than silently
    // serving updates unverified.
    match value.get("signatureVerifier") {
      None | Some(serde_json::Value::Null) => {}
      Some(sv) => match build_signature_verifier(sv) {
        Some(verifier) => {
          config = config.signature(UpdaterSignatureOptions::default().verify(verifier))
        }
        None => return std::ptr::null_mut(),
      },
    }
    Some(config)
  };
  let updater = CoreUpdater::new(source.inner.clone(), remote.inner.clone(), config);
  Box::into_raw(Box::new(WvbUpdater { inner: updater }))
}

/// # Safety
/// `handle` must be null or a pointer previously returned by `wvb_updater_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_free(handle: *mut WvbUpdater) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}

/// # Safety
/// `handle` must be a valid `WvbUpdater`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_list_remotes(handle: *const WvbUpdater) -> *mut WvbResult {
  let Some(updater) = (unsafe { handle.as_ref() }) else {
    return null_handle_err("updater");
  };
  match runtime().block_on(updater.inner.list_remotes()) {
    Ok(list) => ok_result(
      serde_json::Value::Array(list.iter().map(list_info_json).collect()),
      Vec::new(),
    ),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbUpdater`; `bundle_name` a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_get_update(
  handle: *const WvbUpdater,
  bundle_name: *const c_char,
) -> *mut WvbResult {
  let Some(updater) = (unsafe { handle.as_ref() }) else {
    return null_handle_err("updater");
  };
  let name = unsafe { cstr(bundle_name) };
  match runtime().block_on(updater.inner.get_update(&name)) {
    Ok(info) => ok_result(update_info_json(&info), Vec::new()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbUpdater`; `bundle_name` a valid C string; `version` "" = latest.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_download(
  handle: *const WvbUpdater,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let Some(updater) = (unsafe { handle.as_ref() }) else {
    return null_handle_err("updater");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  let version = (!version.is_empty()).then_some(version);
  match runtime().block_on(updater.inner.download(name, version)) {
    Ok(info) => ok_result(remote_info_json(&info), Vec::new()),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be a valid `WvbUpdater`; `bundle_name`/`version` valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_install(
  handle: *const WvbUpdater,
  bundle_name: *const c_char,
  version: *const c_char,
) -> *mut WvbResult {
  let Some(updater) = (unsafe { handle.as_ref() }) else {
    return null_handle_err("updater");
  };
  let name = unsafe { cstr(bundle_name) };
  let version = unsafe { cstr(version) };
  match runtime().block_on(updater.inner.install(name, version)) {
    Ok(()) => ok_result(serde_json::Value::Null, Vec::new()),
    Err(e) => core_err(e),
  }
}
