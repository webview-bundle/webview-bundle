# webview-bundle-tauri

Tauri 2 plugin for [webview-bundle](https://github.com/webview-bundle/webview-bundle) — serve Webview
Bundles through a custom protocol, with builtin + remote bundle sources and updates.

## Installing builtin bundles at build time

Unlike Electron (which has `@wvb/electron-forge` and `@wvb/electron-builder`), Tauri has **no
JS-based plugin or config-wrapping hook**: `tauri.conf.json` is static data, and the only build-time
extension points are the shell `beforeXCommand` hooks plus the static `bundle.resources` list.

So the integration is CLI-driven: stage the bundles with `wvb builtin --tauri` from a build hook, and
declare them in `bundle.resources` so the bundler copies them into the app's Resource directory —
where this plugin reads builtin bundles from at runtime (`<Resource>/bundles`).

### 1. Configure your bundles

`wvb builtin --tauri` reuses your `wvb.config.ts` (same `builtin.target` model as the Electron
integrations — a remote endpoint or local workspaces):

```ts
// wvb.config.ts
export default {
  builtin: { target: { type: 'remote', endpoint: 'https://cdn.example.com' } },
};
```

### 2. Wire the build hook + resources in `tauri.conf.json`

```jsonc
{
  "build": {
    // Stage bundles before the bundler runs, so they are copied into Resources and sealed by signing.
    "beforeBundleCommand": "wvb builtin --tauri"
  },
  "bundle": {
    // Array + glob PRESERVES the bundles/<name>/<name>_<version>.wvb subtree.
    // (Do NOT use the map form `"bundles/**": "bundles"` — it flattens subdirectories.)
    "resources": ["bundles/**/*.wvb", "bundles/manifest.json"]
  }
}
```

`wvb builtin --tauri`:

- Locates the Tauri project (`src-tauri`) even when the hook's working directory is the frontend dir.
- Stages into `<src-tauri>/bundles` so the files land at `<Resource>/bundles` — the plugin's default
  builtin dir, so **no runtime config is needed** for packaged apps.
- Warns if `bundle.resources` doesn't ship the staged bundles (the "staged but not shipped" footgun).

Add the staging dir to `.gitignore` (it holds generated artifacts):

```gitignore
# src-tauri/.gitignore
/bundles
```

## Development

Builtin bundles are a **production** concern only. In `tauri dev` you serve your dev server (e.g.
`http://localhost:1420`) through the `local` protocol and point the webview at it — there is nothing
to install:

```rust
use wvb_tauri::{Config, Protocol};

Config::new().protocol(Protocol::local("app").host("dev", "http://localhost:1420"))
```

So `wvb builtin --tauri` only needs to run at bundle time (`beforeBundleCommand`); `tauri dev` never
bundles, so it never stages or copies bundles, and that's intentional.

## Notes

- `beforeBundleCommand` runs after the Rust compile and before resources are copied, so staged
  bundles are included **and sealed under code-signing/notarization** — never inject bundles after
  bundling.
- The bundles land outside the binary in the Resource directory; the plugin reads them with the
  filesystem, so no special unpacking is required.
