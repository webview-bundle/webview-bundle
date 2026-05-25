# WebViewBundleWebView

System `WKWebView` integration for [WebViewBundle](../../../../..) resources,
built on the UniFFI bindings in `WebViewBundleLibrary`.

It is the iOS/macOS counterpart of the [`@wvb/electron`](../../../../electron)
and [`wvb-tauri`](../../../../tauri) packages: you register one or more
protocols, each bound to a URL scheme, and requests to those schemes are served
from a bundle source (or proxied to a local server) through a
`WKURLSchemeHandler`.

## Install (Swift Package Manager)

Add the package and depend on the `WebViewBundleWebView` product (it re-exports
`WebViewBundleLibrary`):

```swift
.product(name: "WebViewBundleWebView", package: "webview-bundle")
```

## Usage

```swift
import WebKit
import WebViewBundleLibrary
import WebViewBundleWebView

let wvb = WebViewBundle(
    source: BundleSource(config: BundleSourceConfig(
        builtinDir: builtinDir.path,
        remoteDir: remoteDir.path,
        builtinManifestFilepath: nil,
        remoteManifestFilepath: nil
    )),
    protocols: [.bundle(scheme: "app")]
)

// Keep `wvb` alive for the web view's lifetime; it owns the scheme handlers.
let webView = wvb.makeWebView()
webView.load(URLRequest(url: URL(string: "app://app.wvb/index.html")!))
```

To install onto an existing configuration:

```swift
let configuration = WKWebViewConfiguration()
wvb.install(on: configuration)
let webView = WKWebView(frame: .zero, configuration: configuration)
```

### Local protocol

```swift
let wvb = WebViewBundle(
    source: source,
    protocols: [.local(scheme: "local", hosts: ["myapp": "http://localhost:8080"])]
)
```

## Demo

The test app has a working screen: open it, tap **WebView** in the toolbar
(`ios/TestApp/WebViewDemo.swift`). A green heading + JS-rendered origin line
confirms HTML/CSS/JS are served from the bundle.

## Notes

- `WKWebView` only allows scheme handlers for **non-reserved** schemes, so use a
  custom scheme (e.g. `app`, `wvb`) — not `http`/`https`.
- The bundle name is the first label of the host: `app://app.wvb/x` -> `app`.
- WebKit calls the scheme handler on the main thread; the suspending FFI handler
  runs off-main and its result is delivered back on the main thread, skipping
  tasks that were stopped.
