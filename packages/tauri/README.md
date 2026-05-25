# @wvb/tauri

Webview Bundle plugin for [Tauri](https://tauri.app) v2.

Serve your frontend from `.wvb` archives through a custom protocol, ship a set of
**builtin** bundles inside the app, and (optionally) download newer **remote**
bundles at runtime.

## Installation

Add the Rust plugin to `src-tauri/Cargo.toml`:

```toml
[dependencies]
wvb-tauri = "0.1"
```

Add the JavaScript API (for the optional `source` / `remote` / `updater` commands):

```sh
npm install @wvb/tauri
```

## Setup

Register the plugin and the protocols you want to expose:

```rust
use wvb_tauri::{Config, Protocol, Source};

tauri::Builder::default()
  .plugin(wvb_tauri::init(
    Config::new()
      // `bundle://<name>.wvb/...` serves files from a packed bundle.
      .protocol(Protocol::bundle("bundle"))
      // `local://<host>/...` proxies to a dev server (handy for `tauri dev`).
      .protocol(Protocol::local("local").host("example.com", "http://localhost:1420")),
  ))
  .run(tauri::generate_context!())
  .expect("error while running tauri application");
```

Point a window at a bundle in `tauri.conf.json`:

```json
{
  "app": {
    "windows": [{ "url": "bundle://app.wvb" }]
  }
}
```

## Bundle sources

A bundle name is resolved from two directories. **Remote wins over builtin** when
both provide the same bundle, which is what makes runtime updates possible.

| Source    | Purpose                                  | Default directory          | Access     |
| --------- | ---------------------------------------- | -------------------------- | ---------- |
| `builtin` | Bundles shipped with the app (fallback)  | `$RESOURCE/bundles`        | read-only  |
| `remote`  | Bundles downloaded at runtime (priority) | `$APPLOCALDATA/bundles`    | read/write |

Override either directory through `Source`:

```rust
Config::new().source(
  Source::new()
    // Accepts Tauri path variables, e.g. "$RESOURCE/bundles".
    .builtin_dir("$RESOURCE/bundles")
    // Or resolve dynamically from the AppHandle.
    .remote_dir_fn(|app| Ok(app.path().app_local_data_dir()?.join("bundles"))),
)
```

Each directory uses the same on-disk layout, which is exactly what the
`@wvb/cli` tooling produces:

```
bundles/
  manifest.json                 # { manifestVersion, entries: { <name>: { versions, currentVersion } } }
  <name>/<name>_<version>.wvb   # e.g. app/app_1.0.0.wvb
```

## Embedding builtin bundles with `@wvb/cli`

The recommended development flow installs builtin bundles from your remote server
and ships them as Tauri resources.

1. Install builtin bundles into a directory next to `src-tauri`:

   ```sh
   wvb builtin --endpoint https://bundles.example.com --out src-tauri/bundles
   ```

   This downloads the selected bundles and writes `manifest.json` plus
   `<name>/<name>_<version>.wvb` into `src-tauri/bundles`. Use `--include` /
   `--exclude` to filter, or configure `builtin` in your `wvb.config.ts`.

2. Ship that directory as a resource in `tauri.conf.json`:

   ```json
   {
     "bundle": {
       "resources": ["bundles"]
     }
   }
   ```

3. Leave `builtin_dir` at its default. At runtime it resolves to
   `$RESOURCE/bundles`, which is where Tauri unpacks the resource above — so the
   bundles you installed in step 1 are loaded with no extra configuration.

Newer versions can then be fetched at runtime into the writable remote directory
(see below) and will transparently take priority over the builtin copy.

## Remote & updater (optional)

```rust
use wvb_tauri::{Config, Remote};

Config::new()
  .protocol(wvb_tauri::Protocol::bundle("bundle"))
  .remote(Remote::new("https://bundles.example.com"));
```

When a remote is configured, the `remote` and `updater` commands become available
and downloads are written to the remote directory.

## Permissions

Tauri v2 gates every command behind its ACL. Add the plugin's default permission
set to the capability of any window that calls the JavaScript API:

```json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": ["core:default", "wvb-tauri:default"]
}
```

`wvb-tauri:default` grants all commands. To restrict the frontend, reference
individual `wvb-tauri:allow-*` permissions instead
(e.g. `wvb-tauri:allow-source-list-bundles`).

## JavaScript API

```ts
import { source, remote, updater } from '@wvb/tauri/api';

// Inspect what is installed (builtin + remote).
await source.listBundles();
await source.loadVersion('app'); // { type: 'builtin' | 'remote', version }

// Pull updates at runtime.
const info = await updater.getUpdate('app');
if (info.isAvailable) {
  await updater.downloadUpdate('app');
  await source.updateVersion('app', info.version); // switch the active version
}
```
