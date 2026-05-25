# wvb-ffi

UniFFI bindings for the WebViewBundle core, plus system-WebView integrations for
Android and iOS/macOS.

The Rust crate (`src/`) exposes the core types — `BundleSource`,
`BundleUrlHandler`, `LocalUrlHandler`, `Remote`, `Updater`, … — to Kotlin
(`dev.wvb`) and Swift (`WebViewBundleLibrary`) via
[UniFFI](https://mozilla.github.io/uniffi-rs/). On top of those bindings, this
package ships a thin integration layer that plugs WebViewBundle into the system
WebView, mirroring the [`@wvb/electron`](../electron) and [`wvb-tauri`](../tauri)
protocol model.

## Layout

| Path | Description |
| --- | --- |
| `src/` | Rust FFI crate (UniFFI scaffolding). |
| `android/lib-android` | Generated Kotlin bindings + native libraries (`dev.wvb:webview-bundle-ffi`). |
| `android/lib-webview` | **`WebView` integration** (`dev.wvb:webview-bundle`). |
| `apple/` | Generated Swift bindings + `WebViewBundleFFI.xcframework` (`WebViewBundleLibrary`). |
| `apple/Sources/WebViewBundleWebView` | **`WKWebView` integration** (`WebViewBundleWebView`). |

## WebView integration

Both integrations expose a `WebViewBundle` facade. You register protocols — each
bound to a URL scheme — and requests to those schemes are intercepted and served
from a bundle source instead of the network.

- Android: see [`android/lib-webview/README.md`](android/lib-webview/README.md).
- iOS/macOS: see
  [`apple/Sources/WebViewBundleWebView/README.md`](apple/Sources/WebViewBundleWebView/README.md).

The two platforms intercept differently, by necessity:

- **Android** matches on **host over `https`** (e.g. `https://app.wvb/…`). A
  custom scheme would give the page an opaque (`"null"`) origin that breaks
  `localStorage`/`fetch`/cookies/Service Workers, so it serves over a virtual
  `https` host like `WebViewAssetLoader` does.
- **iOS/macOS** uses a **custom scheme** (e.g. `app://app.wvb/…`) registered with
  a `WKURLSchemeHandler`, because `WKWebView` reserves `https`/`http`. Custom
  schemes get a usable origin on iOS.

In both cases the bundle name is the first label of the host
(`app.wvb` -> bundle `app`).

## Building

```sh
yarn workspace @wvb/ffi build-ffi-android   # generates Kotlin bindings + jniLibs
yarn workspace @wvb/ffi build-ffi-apple     # generates Swift bindings + xcframework
```

The integration sources (`android/lib-webview`, `apple/Sources`) are committed;
the generated bindings they depend on are produced by the commands above.
