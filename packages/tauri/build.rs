const COMMANDS: &[&str] = &[
  // source
  "source_list_bundles",
  "source_load_version",
  "source_update_version",
  "source_resolve_filepath",
  "source_get_builtin_bundle_filepath",
  "source_get_remote_bundle_filepath",
  "source_load_builtin_metadata",
  "source_load_remote_metadata",
  "source_unload_descriptor",
  "source_remove_remote_bundle",
  "source_remote_retained_versions",
  "source_prune_remote_bundles",
  // remote
  "remote_list_bundles",
  "remote_get_info",
  "remote_download",
  "remote_download_version",
  // updater
  "updater_list_remotes",
  "updater_get_update",
  "updater_download",
  "updater_install",
];

fn main() {
  tauri_plugin::Builder::new(COMMANDS).build();
}
