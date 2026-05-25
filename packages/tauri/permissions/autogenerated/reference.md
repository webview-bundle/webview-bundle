## Default Permission

Default permissions for the webview bundle plugin.
Grants access to every source, remote and updater command so the `@wvb/tauri`
JavaScript API works out of the box. Reference a narrower set of `allow-*`
permissions instead if you want to restrict what the frontend can invoke.

#### This default permission set includes the following:

- `allow-source-list-bundles`
- `allow-source-load-version`
- `allow-source-update-version`
- `allow-source-filepath`
- `allow-remote-list-bundles`
- `allow-remote-get-info`
- `allow-remote-download`
- `allow-remote-download-version`
- `allow-updater-list-remotes`
- `allow-updater-get-update`
- `allow-updater-download-update`

## Permission Table

<table>
<tr>
<th>Identifier</th>
<th>Description</th>
</tr>


<tr>
<td>

`wvb-tauri:allow-remote-download`

</td>
<td>

Enables the remote_download command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:deny-remote-download`

</td>
<td>

Denies the remote_download command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:allow-remote-download-version`

</td>
<td>

Enables the remote_download_version command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:deny-remote-download-version`

</td>
<td>

Denies the remote_download_version command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:allow-remote-get-info`

</td>
<td>

Enables the remote_get_info command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:deny-remote-get-info`

</td>
<td>

Denies the remote_get_info command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:allow-remote-list-bundles`

</td>
<td>

Enables the remote_list_bundles command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:deny-remote-list-bundles`

</td>
<td>

Denies the remote_list_bundles command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:allow-source-filepath`

</td>
<td>

Enables the source_filepath command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:deny-source-filepath`

</td>
<td>

Denies the source_filepath command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:allow-source-list-bundles`

</td>
<td>

Enables the source_list_bundles command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:deny-source-list-bundles`

</td>
<td>

Denies the source_list_bundles command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:allow-source-load-version`

</td>
<td>

Enables the source_load_version command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:deny-source-load-version`

</td>
<td>

Denies the source_load_version command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:allow-source-update-version`

</td>
<td>

Enables the source_update_version command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:deny-source-update-version`

</td>
<td>

Denies the source_update_version command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:allow-updater-download-update`

</td>
<td>

Enables the updater_download_update command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:deny-updater-download-update`

</td>
<td>

Denies the updater_download_update command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:allow-updater-get-update`

</td>
<td>

Enables the updater_get_update command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:deny-updater-get-update`

</td>
<td>

Denies the updater_get_update command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:allow-updater-list-remotes`

</td>
<td>

Enables the updater_list_remotes command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`wvb-tauri:deny-updater-list-remotes`

</td>
<td>

Denies the updater_list_remotes command without any pre-configured scope.

</td>
</tr>
</table>
