use crate::WebviewBundleExtra;
use tauri::{AppHandle, Runtime, command};
use wvb::remote::{ListRemoteBundleInfo, RemoteBundleInfo, RemoteFetchOptions};
use wvb::source::{BundleManifestVersionData, BundleSourceVersion, ListBundleItem};
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
  let version = wvb.source().get_version(&bundle_name).await?;
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
pub(crate) async fn source_resolve_filepath<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
) -> crate::Result<String> {
  let wvb = app.wvb();
  let filepath = wvb.source().resolve_filepath(&bundle_name).await?;
  Ok(filepath.to_string_lossy().to_string())
}

#[command]
pub(crate) async fn source_get_builtin_bundle_filepath<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
) -> crate::Result<String> {
  let wvb = app.wvb();
  let filepath = wvb
    .source()
    .get_builtin_bundle_filepath(&bundle_name, &version)?;
  Ok(filepath.to_string_lossy().to_string())
}

#[command]
pub(crate) async fn source_get_remote_bundle_filepath<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
) -> crate::Result<String> {
  let wvb = app.wvb();
  let filepath = wvb
    .source()
    .get_remote_bundle_filepath(&bundle_name, &version)?;
  Ok(filepath.to_string_lossy().to_string())
}

#[command]
pub(crate) async fn source_load_builtin_metadata<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
) -> crate::Result<Option<BundleManifestVersionData>> {
  let wvb = app.wvb();
  let metadata = wvb
    .source()
    .get_builtin_metadata(&bundle_name, &version)
    .await?;
  Ok(metadata)
}

#[command]
pub(crate) async fn source_load_remote_metadata<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
) -> crate::Result<Option<BundleManifestVersionData>> {
  let wvb = app.wvb();
  let metadata = wvb
    .source()
    .get_remote_metadata(&bundle_name, &version)
    .await?;
  Ok(metadata)
}

#[command]
pub(crate) async fn source_unload_descriptor<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
) -> crate::Result<bool> {
  let wvb = app.wvb();
  Ok(wvb.source().unload(&bundle_name))
}

#[command]
pub(crate) async fn source_remove_remote_bundle<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
) -> crate::Result<bool> {
  let wvb = app.wvb();
  let removed = wvb
    .source()
    .remove_remote_bundle(&bundle_name, &version)
    .await?;
  Ok(removed)
}

#[command]
pub(crate) async fn source_remote_retained_versions<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
) -> crate::Result<Vec<String>> {
  let wvb = app.wvb();
  let versions = wvb.source().remote_retained_versions(&bundle_name).await?;
  Ok(versions)
}

#[command]
pub(crate) async fn source_prune_remote_bundles<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
) -> crate::Result<Vec<String>> {
  let wvb = app.wvb();
  let removed = wvb.source().prune_remote_bundle(&bundle_name).await?;
  Ok(removed)
}

#[command]
pub(crate) async fn remote_list_bundles<R: Runtime>(
  app: AppHandle<R>,
  options: Option<RemoteFetchOptions>,
) -> crate::Result<Vec<ListRemoteBundleInfo>> {
  let wvb = app.wvb();
  let bundles = wvb
    .remote()
    .ok_or(crate::Error::RemoteIsNotInitialized)?
    .list_bundles(options)
    .await?;
  Ok(bundles)
}

#[command]
pub(crate) async fn remote_get_info<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  options: Option<RemoteFetchOptions>,
) -> crate::Result<RemoteBundleInfo> {
  let wvb = app.wvb();
  let info = wvb
    .remote()
    .ok_or(crate::Error::RemoteIsNotInitialized)?
    .get_current_info(&bundle_name, options)
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
pub(crate) async fn updater_download<R: Runtime>(
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

#[command]
pub(crate) async fn updater_install<R: Runtime>(
  app: AppHandle<R>,
  bundle_name: String,
  version: String,
) -> crate::Result<()> {
  let wvb = app.wvb();
  wvb
    .updater()
    .ok_or(crate::Error::UpdaterIsNotInitialized)?
    .install(bundle_name, version)
    .await?;
  Ok(())
}
