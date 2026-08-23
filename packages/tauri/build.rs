const COMMANDS: &[&str] = &[
  // source
  "source_list_bundles",
  "source_list_builtin_bundles",
  "source_list_remote_bundles",
  "source_get_version",
  "source_get_remote_staged_version",
  "source_get_remote_previous_version",
  "source_get_builtin_version_data",
  "source_get_remote_version_data",
  "source_update_remote_version",
  "source_update_remote_versions",
  "source_stage_remote_bundle",
  "source_stage_remote_bundles",
  "source_remove_remote_bundle",
  "source_remove_remote_bundles",
  "source_prune_remote_bundle",
  "source_prune_remote_bundles",
  "source_resolve_filepath",
  "source_get_builtin_bundle_filepath",
  "source_get_remote_bundle_filepath",
  "source_unload",
  // remote
  "remote_get_update",
  "remote_download",
  // updater
  "updater_get_update",
  "updater_download",
  "updater_install",
  "updater_rollback",
];

fn main() {
  tauri_plugin::Builder::new(COMMANDS).build();
}
