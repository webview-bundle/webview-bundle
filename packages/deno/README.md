# @wvb/deno

[webview-bundle](https://github.com/webview-bundle/webview-bundle) core API bound to the **Deno**
runtime via [Deno FFI](https://docs.deno.com/runtime/fundamentals/ffi/) — the Deno peer of
[`@wvb/node`](https://github.com/webview-bundle/webview-bundle/tree/main/packages/node). Exposes
`BundleSource`, `BundleProtocol`, `LocalProtocol`, `Remote`, `Updater` (+ `toResponse`), backed by
the `wvb-deno` cdylib through `Deno.dlopen`.

For Deno **desktop** apps, use
[`@wvb/deno-desktop`](https://github.com/webview-bundle/webview-bundle/tree/main/packages/deno-desktop),
which wraps this into a `Deno.serve` handler.

## Loading the native library

The cdylib (`.dylib`/`.so`/`.dll`) is loaded one of three ways — call before using any binding:

```ts
import { loadLib, loadLibViaPlug } from '@wvb/deno';

// 1) explicit path — deno desktop (--include'd dylib) or any known location.
//    Pass a file or a directory (the platform filename is appended).
loadLib(new URL('./vendor/wvb/', import.meta.url));

// 2) local dev — set WVB_DENO_LIB=target/debug/libwvb_deno.dylib; the binding auto-loads from it.

// 3) deno run / library — download + cache from a release URL via @denosaurs/plug.
await loadLibViaPlug({
  url: 'https://github.com/webview-bundle/webview-bundle/releases/download/deno@<version>/',
});
```

Required permissions: `--allow-ffi --allow-read --allow-env` (`--allow-net` if downloading via Plug
or using `Remote`/`Updater`). If you see "native library not loaded", call `loadLib`/`loadLibViaPlug`
first.

## `wvb builtin`-style install (for `deno desktop --include`)

`deno desktop`/`deno compile` don't bundle FFI dylibs automatically — vendor it first:

```sh
deno run -A jsr:@wvb/deno/install --out vendor/wvb [--target <triple>]
# → vendor/wvb/libwvb_deno.dylib ; then:
deno desktop --allow-ffi --include vendor/wvb/libwvb_deno.dylib main.ts
```

The dylib is hosted per-platform on GitHub Releases (JSR ships only the TS).

## API

`BundleSource` · `BundleProtocol` (serve bundle files: GET/HEAD, content-type, Range/206,
`index.html` fallback) · `LocalProtocol` (proxy to a dev server) · `Remote` (listBundles/getInfo/
download/downloadVersion) · `Updater` (listRemotes/getUpdate/download/install) · `toResponse`
(HttpResponse → web `Response`) · `loadLib`/`loadLibViaPlug`/`platformLibFileName`.

> Status: experimental. The bundle codec (`Bundle`, `BundleBuilder`, …) is not yet bound.
