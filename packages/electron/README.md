# @wvb/electron

Webview Bundle API for Electron.

## Native binaries

`@wvb/electron` bundles the `@wvb/node` native binary for every desktop target it
supports (macOS x64/arm64, Windows x64/arm64/ia32, Linux x64/arm64 glibc) under
`native/`, and at import time points `@wvb/node`'s loader at the one matching the
current process. This avoids relying on `@wvb/node`'s per-arch optional dependencies,
which Electron packagers routinely fail to unpack and which cannot support universal
or cross-arch builds.

Two things to keep in mind when packaging an app:

- **Keep `@wvb/electron` external in your bundler.** It ships `.node` files and loads
  them at runtime, so it must not be inlined into your main-process bundle. In Vite,
  add it to `build.rollupOptions.external` (e.g. `/^@wvb\//`); most Electron bundler
  presets already externalize native dependencies. Ship it unbundled in the packaged
  app's `node_modules` with its `dist/` and `native/` directories intact, and unpack
  the `.node` files from asar (e.g. electron-builder `asarUnpack: "**/*.node"`).

- **Import `@wvb/electron` before `@wvb/node`.** The bundled-binary override only
  applies if it runs before `@wvb/node`'s binding first loads. `@wvb/electron`
  re-exports the helpers you would normally reach into `@wvb/node` for
  (`isWebviewBundleError`, `WebviewBundleError`), so prefer importing those from
  `@wvb/electron`.

To force a specific binary regardless of the bundled ones, set the
`NAPI_RS_NATIVE_LIBRARY_PATH` environment variable before the app starts; an existing
value is always respected.
