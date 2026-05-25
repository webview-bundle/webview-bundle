const COMMANDS: &[&str] = &[
  "source_list_bundles",
  "source_load_version",
  "source_update_version",
  "source_filepath",
  "remote_list_bundles",
  "remote_get_info",
  "remote_download",
  "remote_download_version",
  "updater_list_remotes",
  "updater_get_update",
  "updater_download_update",
];

fn main() {
  tauri_plugin::Builder::new(COMMANDS).build();
}
