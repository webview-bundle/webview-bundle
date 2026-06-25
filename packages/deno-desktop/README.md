# @wvb/deno-desktop

Serve [webview-bundle](https://github.com/webview-bundle/webview-bundle) `.wvb` bundles in a
[**Deno desktop**](https://docs.deno.com/runtime/desktop/) app. The API mirrors
[`@wvb/electron`](https://github.com/webview-bundle/webview-bundle/tree/main/packages/electron) — `webviewBundle`, `bundleProtocol`, `localProtocol`, `bundleSource`,
`remote` — adapted to Deno desktop's model: the webview points at a local `Deno.serve` endpoint (no
custom protocol), so a `WebviewBundle` exposes a `fetch` handler instead of registering protocols.

Built on the [`@wvb/deno`](https://github.com/webview-bundle/webview-bundle/tree/main/packages/deno) FFI binding, which needs the native `wvb-deno` dynamic library.

## Usage

```ts
import { bundleProtocol, webviewBundle } from '@wvb/deno-desktop';

const app = webviewBundle({
  // deno desktop --include'd dylib, resolved from THIS app module's import.meta.url:
  lib: new URL('./libwvb_deno.dylib', import.meta.url),
  protocols: [bundleProtocol('app')], // serve builtin bundle "app" at the HTTP root
});

Deno.serve(app.fetch); // the runtime owns the port; the webview navigates to "/"
```

`webviewBundle(config)` returns a `WebviewBundle` with `.fetch` (the `Deno.serve` handler) and
`.source` / `.remote` / `.updater` / `.protocolSchemes` getters. `wvb` is an alias for
`webviewBundle`. Because Deno desktop is single-origin, the **first** protocol is served at the root,
and a protocol's `scheme` is the bundle name / local host (vs. a URL scheme in Electron).

### Where bundles live

| | location | default |
| --- | --- | --- |
| **builtin** (read-only, shipped) | inside the app bundle | `bundles` next to the entry module (`Deno.mainModule`) — the `--include`d, self-extracted dir |
| **remote** (downloaded, writable) | OS application-data dir | `<app-data>/<appName>/bundles` |

So you usually don't pass either path — `bundleSource` resolves builtin from the app bundle and
remote from the app-data directory (`appDataDir()`: macOS `~/Library/Application Support`, Windows
`%APPDATA%`, Linux `$XDG_DATA_HOME`; override with `WVB_APP_DATA_DIR`). `appName` (default: the
executable name) names the per-app subfolder. Both are overridable via `source.builtinDir` /
`source.remoteDir`.

### With an updater (OTA)

```ts
const app = webviewBundle({
  lib: new URL('./libwvb_deno.dylib', import.meta.url),
  // builtin read from the app bundle; downloads persist under <app-data>/myapp/bundles
  source: { appName: 'myapp' },
  updater: { remote: { endpoint: 'https://cdn.example.com' }, channel: 'stable' },
  protocols: [bundleProtocol('app')],
});
await app.updater?.getUpdate('app');
```

### Dev mode (proxy a dev server)

```ts
const app = webviewBundle({
  lib: Deno.env.get('WVB_DENO_LIB')!, // cargo-built dylib during dev
  protocols: [localProtocol('app', { hosts: { app: 'http://localhost:5173' } })],
});
Deno.serve(app.fetch);
```

## Bundling the native library + bundles

`deno desktop` (= `deno compile`) does **not** embed FFI dylibs automatically. Vendor the dylib, then
`--include` it (and your bundles). Both are resolved at runtime from the app's `import.meta.url`.

```sh
# 1. vendor the cdylib for your build target into the project
deno run -A jsr:@wvb/deno/install --out . # writes ./libwvb_deno.dylib

# 2. build (deno desktop, or deno compile which it builds on)
deno compile --allow-ffi --allow-read --allow-net --allow-write \
  --self-extracting --include libwvb_deno.dylib --include bundles \
  --output app main.ts
```

**`--self-extracting` is required for the bundles.** `--include`d files normally live in Deno's
in-memory VFS, but the native cdylib reads the real filesystem — so it can't see VFS bundles.
`--self-extracting` extracts the embedded files to disk on first run, after which `import.meta.url`
paths point at real files the cdylib can read. (Verified with `deno compile` on Deno 2.7; the
`deno desktop` window itself needs Deno 2.9+.)

> **Monorepo / local dev caveat:** `deno compile` loads modules redirected via a `file://` import map
> **raw** (untranspiled), and the binary crashes on TS syntax. Resolve `@wvb/deno-desktop` through a
> Deno **workspace** (or a relative import) so `deno compile` transpiles + embeds it. Normal JSR
> consumers are unaffected.

## `deno run` (non-bundled)

For plain `deno run` (no embedded dylib), pre-load the library from a release URL via
`@denosaurs/plug` and omit `lib`:

```ts
import { loadLibViaPlug, webviewBundle, bundleProtocol } from '@wvb/deno-desktop';

await loadLibViaPlug({ url: 'https://github.com/.../releases/download/deno@<version>/' });
const app = webviewBundle({ source: { builtinDir }, protocols: [bundleProtocol('app')] });
Deno.serve(app.fetch);
```

> Windows note: embedded FFI dylibs via `deno compile` have a known dlopen issue
> ([denoland/deno#31218](https://github.com/denoland/deno/issues/31218)). Status: experimental.
