use crate::WebviewBundleExtra;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, Runtime, command};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleUpdate {
  pub name: String,
  pub version: String,
  pub download_url: Option<String>,
  pub integrity: Option<String>,
  pub metadata: Option<HashMap<String, String>>,
}

impl From<wvb::remote::BundleUpdate> for BundleUpdate {
  fn from(value: wvb::remote::BundleUpdate) -> Self {
    Self {
      name: value.name,
      version: value.version,
      download_url: value.download_url,
      integrity: value.integrity,
      metadata: value.metadata,
    }
  }
}

impl From<BundleUpdate> for wvb::remote::BundleUpdate {
  fn from(value: BundleUpdate) -> Self {
    Self {
      name: value.name,
      version: value.version,
      download_url: value.download_url,
      integrity: value.integrity,
      metadata: value.metadata,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
  pub id: String,
  pub created_at: String,
  pub runtime_version: u8,
  pub bundles: Vec<BundleUpdate>,
  pub metadata: HashMap<String, String>,
}

impl From<wvb::remote::Update> for Update {
  fn from(value: wvb::remote::Update) -> Self {
    Self {
      id: value.id,
      created_at: value.created_at,
      runtime_version: value.runtime_version,
      bundles: value.bundles.into_iter().map(BundleUpdate::from).collect(),
      metadata: value.metadata,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSignature {
  pub key_id: String,
  pub sig: String,
  pub alg: String,
}

impl From<wvb::remote::UpdateSignature> for UpdateSignature {
  fn from(value: wvb::remote::UpdateSignature) -> Self {
    Self {
      key_id: value.key_id,
      sig: value.sig,
      alg: value.alg,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteUpdateResponse {
  pub update: Update,
  pub etag: Option<String>,
  pub signature: Option<UpdateSignature>,
}

impl From<wvb::remote::RemoteUpdateResponse> for RemoteUpdateResponse {
  fn from(value: wvb::remote::RemoteUpdateResponse) -> Self {
    Self {
      update: value.update.into(),
      etag: value.etag,
      signature: value.signature.map(UpdateSignature::from),
    }
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteGetUpdateOptions {
  pub etag: Option<String>,
  pub channel: Option<String>,
}

impl From<RemoteGetUpdateOptions> for wvb::remote::RemoteGetUpdateOptions {
  fn from(value: RemoteGetUpdateOptions) -> Self {
    let mut options = wvb::remote::RemoteGetUpdateOptions::default();
    if let Some(etag) = value.etag {
      options = options.etag(etag);
    }
    if let Some(channel) = value.channel {
      options = options.channel(channel);
    }
    options
  }
}

#[command]
pub async fn remote_get_update<R: Runtime>(
  app: AppHandle<R>,
  options: Option<RemoteGetUpdateOptions>,
) -> crate::Result<Option<RemoteUpdateResponse>> {
  let wvb = app.wvb();
  let resp = wvb
    .remote()
    .ok_or(crate::Error::RemoteIsNotInitialized)?
    .get_update(options.map(Into::into))
    .await?
    .map(RemoteUpdateResponse::from);
  Ok(resp)
}

#[command]
pub async fn remote_download<R: Runtime>(
  app: AppHandle<R>,
  url: String,
  filepath: PathBuf,
) -> crate::Result<()> {
  let wvb = app.wvb();
  wvb
    .remote()
    .ok_or(crate::Error::RemoteIsNotInitialized)?
    .download(url, &filepath, None)
    .await?;
  Ok(())
}
