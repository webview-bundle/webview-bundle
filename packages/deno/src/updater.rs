#![allow(dead_code)]

use crate::cancellation::{WvbCancellation, cancellation_of};
use crate::error::ErrorCode;
use crate::integrity::{IntegrityAlgorithm, IntegrityPolicy};
use crate::remote::{BundleUpdate, Update, WvbRemote};
use crate::result::{WvbResult, core_err, err_result, null_handle_err, ok_handle, ok_result};
use crate::signature::SignatureVerifyKey;
use crate::source::WvbSource;
use crate::{cstr, runtime};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::ffi::c_char;
use std::path::Path;
use wvb::updater;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdaterIntegrityOptions {
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub policy: Option<IntegrityPolicy>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub algorithm: Option<IntegrityAlgorithm>,
}

impl From<UpdaterIntegrityOptions> for updater::UpdaterIntegrityOptions {
  fn from(value: UpdaterIntegrityOptions) -> Self {
    let mut options = Self::default();
    if let Some(policy) = value.policy {
      options = options.policy(policy.into());
    }
    if let Some(algorithm) = value.algorithm {
      options = options.algorithm(algorithm.into());
    }
    options
  }
}

/// The keys an update response may be signed with, each published under its own id.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdaterSignatureOptions {
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub keys: Option<Vec<SignatureVerifyKey>>,
}

impl TryFrom<UpdaterSignatureOptions> for updater::UpdaterSignatureOptions {
  type Error = String;

  fn try_from(value: UpdaterSignatureOptions) -> Result<Self, Self::Error> {
    let mut options = Self::default();
    if let Some(keys) = value.keys {
      let keys = keys
        .iter()
        .map(wvb::signature::SignatureVerifyKey::try_from)
        .collect::<Result<Vec<_>, _>>()?;
      options = options.add_keys(keys);
    }
    Ok(options)
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdaterOptions {
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub channel: Option<String>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub integrity: Option<UpdaterIntegrityOptions>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub signature: Option<UpdaterSignatureOptions>,
}

impl TryFrom<UpdaterOptions> for updater::UpdaterOptions {
  type Error = String;

  fn try_from(value: UpdaterOptions) -> Result<Self, Self::Error> {
    let mut options = Self::default();
    if let Some(channel) = value.channel {
      options = options.channel(channel);
    }
    if let Some(integrity) = value.integrity {
      options = options.integrity(integrity.into());
    }
    if let Some(signature) = value.signature {
      options = options.signature(signature.try_into()?);
    }
    Ok(options)
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdaterGetUpdateOptions {
  /// Require the update response to be signed by the key published under this id.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub expect_signature_key_id: Option<String>,
}

impl From<UpdaterGetUpdateOptions> for updater::UpdaterGetUpdateOptions {
  fn from(value: UpdaterGetUpdateOptions) -> Self {
    let mut options = Self::default();
    if let Some(key_id) = value.expect_signature_key_id {
      options = options.expect_signature_key_id(key_id);
    }
    options
  }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdaterDownloadOptions {
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub concurrency: Option<u32>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub timeout: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdaterDownloadResultKind {
  Downloaded,
  Error { code: String, message: String },
}

impl From<updater::UpdaterDownloadResultKind> for UpdaterDownloadResultKind {
  fn from(value: updater::UpdaterDownloadResultKind) -> Self {
    match value {
      updater::UpdaterDownloadResultKind::Downloaded => Self::Downloaded,
      updater::UpdaterDownloadResultKind::Error(e) => Self::Error {
        code: ErrorCode::from(e.code()).as_str().to_string(),
        message: e.to_string(),
      },
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterDownloadResult {
  pub name: String,
  pub version: String,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub integrity: Option<String>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub metadata: Option<HashMap<String, String>>,
  pub result: UpdaterDownloadResultKind,
}

impl From<updater::UpdaterDownloadResult> for UpdaterDownloadResult {
  fn from(value: updater::UpdaterDownloadResult) -> Self {
    Self {
      name: value.name,
      version: value.version,
      integrity: value.integrity,
      metadata: value.metadata,
      result: value.result.into(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdaterInstallTarget {
  pub name: String,
  /// The staged version to install. When omitted, the staged version recorded in the manifest is
  /// used; when given, it has to match that staged version.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub version: Option<String>,
}

impl From<UpdaterInstallTarget> for updater::UpdaterInstallTarget {
  fn from(value: UpdaterInstallTarget) -> Self {
    Self {
      name: value.name,
      version: value.version,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdaterInstallResultKind {
  Installed,
  StagedVersionNotMatched,
  StagedBundleNotExists,
  VerifyFailed,
  Error { code: String, message: String },
}

impl From<updater::UpdaterInstallResultKind> for UpdaterInstallResultKind {
  fn from(value: updater::UpdaterInstallResultKind) -> Self {
    match value {
      updater::UpdaterInstallResultKind::Installed => Self::Installed,
      updater::UpdaterInstallResultKind::StagedVersionNotMatched => Self::StagedVersionNotMatched,
      updater::UpdaterInstallResultKind::StagedBundleNotExists => Self::StagedBundleNotExists,
      updater::UpdaterInstallResultKind::VerifyFailed => Self::VerifyFailed,
      updater::UpdaterInstallResultKind::Error(e) => Self::Error {
        code: ErrorCode::from(e.code()).as_str().to_string(),
        message: e.to_string(),
      },
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterInstallResult {
  pub name: String,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub target_version: Option<String>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub install_version: Option<String>,
  pub result: UpdaterInstallResultKind,
}

impl From<updater::UpdaterInstallResult> for UpdaterInstallResult {
  fn from(value: updater::UpdaterInstallResult) -> Self {
    Self {
      name: value.name,
      target_version: value.target_version,
      install_version: value.install_version,
      result: value.result.into(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdaterRollbackTarget {
  pub name: String,
  /// The previous version to roll back to. When omitted, the previous version recorded in the
  /// manifest is used; when given, it has to match that previous version.
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub version: Option<String>,
}

impl From<UpdaterRollbackTarget> for updater::UpdaterRollbackTarget {
  fn from(value: UpdaterRollbackTarget) -> Self {
    Self {
      name: value.name,
      version: value.version,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdaterRollbackResultKind {
  RolledBack,
  PreviousVersionNotMatched,
  PreviousBundleNotExists,
  VerifyFailed,
  Error { code: String, message: String },
}

impl From<updater::UpdaterRollbackResultKind> for UpdaterRollbackResultKind {
  fn from(value: updater::UpdaterRollbackResultKind) -> Self {
    match value {
      updater::UpdaterRollbackResultKind::RolledBack => Self::RolledBack,
      updater::UpdaterRollbackResultKind::PreviousVersionNotMatched => {
        Self::PreviousVersionNotMatched
      }
      updater::UpdaterRollbackResultKind::PreviousBundleNotExists => Self::PreviousBundleNotExists,
      updater::UpdaterRollbackResultKind::VerifyFailed => Self::VerifyFailed,
      updater::UpdaterRollbackResultKind::Error(e) => Self::Error {
        code: ErrorCode::from(e.code()).as_str().to_string(),
        message: e.to_string(),
      },
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterRollbackResult {
  pub name: String,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub target_version: Option<String>,
  #[specta(optional)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub rollback_version: Option<String>,
  pub result: UpdaterRollbackResultKind,
}

impl From<updater::UpdaterRollbackResult> for UpdaterRollbackResult {
  fn from(value: updater::UpdaterRollbackResult) -> Self {
    Self {
      name: value.name,
      target_version: value.target_version,
      rollback_version: value.rollback_version,
      result: value.result.into(),
    }
  }
}

pub struct WvbUpdater {
  inner: updater::Updater,
}

macro_rules! updater_of {
  ($handle:expr) => {
    match unsafe { $handle.as_ref() } {
      Some(handle) => &handle.inner,
      None => return null_handle_err("updater"),
    }
  };
}

fn json_result<T: Serialize>(value: T) -> *mut WvbResult {
  match serde_json::to_value(value) {
    Ok(json) => ok_result(json, Vec::new()),
    Err(e) => err_result(ErrorCode::CoreSerdeJson, e.to_string()),
  }
}

/// Create an updater over a source + remote. `options_json` is null/empty or an `UpdaterOptions`
/// object; a key it cannot build fails the call rather than serving updates unverified.
///
/// # Safety
/// `source`/`remote` must be valid handles; `update_filepath` a valid C string; `options_json` null
/// or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_new(
  source: *const WvbSource,
  remote: *const WvbRemote,
  update_filepath: *const c_char,
  options_json: *const c_char,
) -> *mut WvbResult {
  let Some(source) = (unsafe { source.as_ref() }) else {
    return null_handle_err("source");
  };
  let Some(remote) = (unsafe { remote.as_ref() }) else {
    return null_handle_err("remote");
  };
  let update_filepath = unsafe { cstr(update_filepath) };
  let mut builder = updater::Updater::builder()
    .source(source.inner.clone())
    .remote(remote.inner.clone())
    .update_filepath(Path::new(&update_filepath));
  let raw = unsafe { cstr(options_json) };
  if !raw.is_empty() {
    let options: UpdaterOptions = match serde_json::from_str(&raw) {
      Ok(options) => options,
      Err(e) => {
        return err_result(
          ErrorCode::InvalidRequest,
          format!("invalid updater options: {e}"),
        );
      }
    };
    match updater::UpdaterOptions::try_from(options) {
      Ok(options) => builder = builder.options(options),
      Err(message) => return err_result(ErrorCode::InvalidSignatureKey, message),
    }
  }
  match builder.build() {
    Ok(updater) => ok_handle(Box::into_raw(Box::new(WvbUpdater { inner: updater }))),
    Err(e) => core_err(e),
  }
}

/// # Safety
/// `handle` must be null or a pointer previously returned by `wvb_updater_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_free(handle: *mut WvbUpdater) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}

/// The bundles this source is missing, or `null` when it is already up to date.
///
/// # Safety
/// `handle` must be a valid `WvbUpdater`; `options_json` null or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_get_update(
  handle: *const WvbUpdater,
  options_json: *const c_char,
) -> *mut WvbResult {
  let updater = updater_of!(handle);
  let raw = unsafe { cstr(options_json) };
  let options = if raw.is_empty() {
    None
  } else {
    match serde_json::from_str::<UpdaterGetUpdateOptions>(&raw) {
      Ok(options) => Some(options.into()),
      Err(e) => {
        return err_result(
          ErrorCode::InvalidRequest,
          format!("invalid get update options: {e}"),
        );
      }
    }
  };
  match runtime().block_on(updater.get_update(options)) {
    Ok(update) => json_result(update.map(Update::from)),
    Err(e) => core_err(e),
  }
}

/// Download the given bundle updates. Downloading only stages them on disk; installing is what
/// activates them.
///
/// # Safety
/// `handle` must be a valid `WvbUpdater`; `bundle_updates_json`/`options_json` valid C strings;
/// `cancellation` null or a valid `WvbCancellation`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_updater_download(
  handle: *const WvbUpdater,
  bundle_updates_json: *const c_char,
  options_json: *const c_char,
  cancellation: *const WvbCancellation,
) -> *mut WvbResult {
  let updater = updater_of!(handle);
  let raw = unsafe { cstr(bundle_updates_json) };
  let bundle_updates: Vec<BundleUpdate> = match serde_json::from_str(&raw) {
    Ok(updates) => updates,
    Err(e) => {
      return err_result(
        ErrorCode::InvalidRequest,
        format!("invalid bundle updates: {e}"),
      );
    }
  };
  let bundle_updates = bundle_updates
    .into_iter()
    .map(Into::into)
    .collect::<Vec<wvb::remote::BundleUpdate>>();

  let raw = unsafe { cstr(options_json) };
  let mut options = updater::UpdaterDownloadOptions::default();
  if !raw.is_empty() {
    let parsed: UpdaterDownloadOptions = match serde_json::from_str(&raw) {
      Ok(options) => options,
      Err(e) => {
        return err_result(
          ErrorCode::InvalidRequest,
          format!("invalid download options: {e}"),
        );
      }
    };
    if let Some(concurrency) = parsed.concurrency {
      options = options.concurrency(concurrency as usize);
    }
    if let Some(timeout) = parsed.timeout {
      options = options.timeout(timeout as u64);
    }
  }
  if let Some(cancellation) = unsafe { cancellation_of(cancellation) } {
    options = options.cancellation(cancellation);
  }

  match runtime().block_on(updater.download(&bundle_updates, Some(options))) {
    Ok(results) => json_result(
      results
        .into_iter()
        .map(UpdaterDownloadResult::from)
        .collect::<Vec<_>>(),
    ),
    Err(e) => core_err(e),
  }
}

/// A `(targets_json) -> Vec<result>` updater call.
macro_rules! targets_call {
  ($name:ident, $method:ident, $target:ty, $result:ty, $what:literal) => {
    /// # Safety
    /// `handle` must be a valid `WvbUpdater`; `targets_json` a valid C string.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn $name(
      handle: *const WvbUpdater,
      targets_json: *const c_char,
    ) -> *mut WvbResult {
      let updater = updater_of!(handle);
      let raw = unsafe { cstr(targets_json) };
      let targets: Vec<$target> = match serde_json::from_str(&raw) {
        Ok(targets) => targets,
        Err(e) => {
          return err_result(ErrorCode::InvalidRequest, format!("invalid {}: {e}", $what));
        }
      };
      let targets = targets.into_iter().map(Into::into).collect::<Vec<_>>();
      match runtime().block_on(updater.$method(&targets)) {
        Ok(results) => json_result(results.into_iter().map(<$result>::from).collect::<Vec<_>>()),
        Err(e) => core_err(e),
      }
    }
  };
}

targets_call!(
  wvb_updater_install,
  install,
  UpdaterInstallTarget,
  UpdaterInstallResult,
  "install targets"
);
targets_call!(
  wvb_updater_rollback,
  rollback,
  UpdaterRollbackTarget,
  UpdaterRollbackResult,
  "rollback targets"
);
