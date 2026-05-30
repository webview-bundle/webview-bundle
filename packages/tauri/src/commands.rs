use crate::WebviewBundleExtra;
use tauri::{AppHandle, Runtime, command};
use wvb::remote::{ListRemoteBundleInfo, RemoteBundleInfo};
use wvb::source::{BundleSourceVersion, ListBundleItem};
use wvb::updater::BundleUpdateInfo;

#[command]
pub(crate) async fn source_list_bundles<R: Runtime>(
  app: AppHandle<R>,
) -> crate::Result<Vec<ListBundleItem>> {
  let wvb = app.wvb();
  let bundles = wvb.source().list_bundles().await?;
  Ok(bundles)
}

#[command]
pub(crate) async fn source_load_version<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
) -> crate::Result<Option<BundleSourceVersion>> {
  let wvb = app.wvb();
  let version = wvb.source().load_version(&bundle_name).await?;
  Ok(version)
}

#[command]
pub(crate) async fn source_update_version<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
) -> crate::Result<()> {
  let wvb = app.wvb();
  wvb
    .source()
    .update_remote_version(&bundle_name, &version)
    .await?;
  Ok(())
}

#[command]
pub(crate) async fn source_filepath<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
) -> crate::Result<String> {
  let wvb = app.wvb();
  let filepath = wvb.source().bundle_filepath(&bundle_name).await?;
  Ok(filepath.to_string_lossy().to_string())
}

#[command]
pub(crate) async fn remote_list_bundles<R: Runtime>(
  app: AppHandle<R>,
  channel: Option<String>,
) -> crate::Result<Vec<ListRemoteBundleInfo>> {
  let wvb = app.wvb();
  let bundles = wvb
    .remote()
    .ok_or(crate::Error::RemoteIsNotInitialized)?
    .list_bundles(channel.as_ref())
    .await?;
  Ok(bundles)
}

#[command]
pub(crate) async fn remote_get_info<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  channel: Option<String>,
) -> crate::Result<RemoteBundleInfo> {
  let wvb = app.wvb();
  let info = wvb
    .remote()
    .ok_or(crate::Error::RemoteIsNotInitialized)?
    .get_current_info(&bundle_name, channel.as_ref())
    .await?;
  Ok(info)
}

#[command]
pub(crate) async fn remote_download<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  channel: Option<String>,
) -> crate::Result<RemoteBundleInfo> {
  let wvb = app.wvb();
  let (info, _, _) = wvb
    .remote()
    .ok_or(crate::Error::RemoteIsNotInitialized)?
    .download(&bundle_name, channel.as_ref())
    .await?;
  Ok(info)
}

#[command]
pub(crate) async fn remote_download_version<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
) -> crate::Result<RemoteBundleInfo> {
  let wvb = app.wvb();
  let (info, _, _) = wvb
    .remote()
    .ok_or(crate::Error::RemoteIsNotInitialized)?
    .download_version(&bundle_name, &version)
    .await?;
  Ok(info)
}

#[command]
pub(crate) async fn updater_list_remotes<R: Runtime>(
  app: AppHandle<R>,
) -> crate::Result<Vec<ListRemoteBundleInfo>> {
  let wvb = app.wvb();
  let bundles = wvb
    .updater()
    .ok_or(crate::Error::UpdaterIsNotInitialized)?
    .list_remotes()
    .await?;
  Ok(bundles)
}

#[command]
pub(crate) async fn updater_get_update<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
) -> crate::Result<BundleUpdateInfo> {
  let wvb = app.wvb();
  let info = wvb
    .updater()
    .ok_or(crate::Error::UpdaterIsNotInitialized)?
    .get_update(&bundle_name)
    .await?;
  Ok(info)
}

#[command]
pub(crate) async fn updater_download_update<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: Option<String>,
) -> crate::Result<RemoteBundleInfo> {
  let wvb = app.wvb();
  let info = wvb
    .updater()
    .ok_or(crate::Error::UpdaterIsNotInitialized)?
    .download(bundle_name, version)
    .await?;
  Ok(info)
}
