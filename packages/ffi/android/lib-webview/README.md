# WebViewBundle for Android

System `WebView` integration for [WebViewBundle](../../../..) resources, built on
the UniFFI bindings in `:lib-android`.

It is the Android counterpart of the [`@wvb/electron`](../../../electron) and
[`wvb-tauri`](../../../tauri) packages: you register one or more protocols, each
bound to a virtual **host**, and requests to those hosts over `https` are served
from a bundle source (or proxied to a local server) through
`WebViewClient.shouldInterceptRequest`.

> **Why `https`, not a custom scheme?** On Android, a custom scheme (`app://…`)
> gives the page an opaque (`"null"`) origin, which breaks `localStorage`,
> `fetch`, cookies, and Service Workers. Serving over a virtual `https` host —
> the same approach as `WebViewAssetLoader` — keeps a real secure origin. The
> bundle name is the first label of the host (`https://app.wvb/x` -> `app`), and
> requests to unregistered hosts fall through to the network unchanged.
> (iOS is the opposite: `WKWebView` reserves `https`/`http`, so it uses a custom
> scheme — see [`WebViewBundleWebView`](../../apple/Sources/WebViewBundleWebView/README.md).)

## Install (Gradle)

The artifact is `dev.wvb:webview-bundle`. It transitively pulls the FFI bindings
and native libraries (`dev.wvb:webview-bundle-ffi`).

```kotlin
dependencies {
    implementation("dev.wvb:webview-bundle:<version>")
}
```

> Until the library is published to a public Maven repository you can consume it
> locally: run `./gradlew :lib-webview:publishToMavenLocal` (after building the
> bindings with `yarn workspace @wvb/ffi build-ffi-android`) and add
> `mavenLocal()` to your repositories.

## Usage

```kotlin
import dev.wvb.BundleSource
import dev.wvb.BundleSourceConfig
import dev.wvb.webview.Protocol
import dev.wvb.webview.WebViewBundle
import dev.wvb.webview.WebViewBundleAssets

// Builtin bundles ship in `assets/bundles/builtin`; copy them to a real path.
val builtinDir = WebViewBundleAssets.copyAssetDir(
    context,
    assetPath = "bundles/builtin",
    destDir = File(context.filesDir, "builtin"),
)
val remoteDir = WebViewBundleAssets.defaultRemoteDir(context)

val wvb = WebViewBundle(
    source = BundleSource(
        BundleSourceConfig(
            builtinDir = builtinDir.absolutePath,
            remoteDir = remoteDir.absolutePath,
            builtinManifestFilepath = null,
            remoteManifestFilepath = null,
        )
    ),
    protocols = listOf(Protocol.Bundle("app.wvb")),
)

wvb.install(webView)                          // sets webView.webViewClient
webView.loadUrl("https://app.wvb/index.html") // host label "app" -> bundle "app"
```

### Combining with your own `WebViewClient`

`intercept` returns `null` for unhandled schemes, so you can call it from an
existing client or subclass `WebViewBundleClient`:

```kotlin
class MyClient(wvb: WebViewBundle) : WebViewBundleClient(wvb) {
    override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest) =
        super.shouldInterceptRequest(view, request) ?: myFallback(request)
}
```

### Local protocol

```kotlin
// Useful for local development: serve https://app.wvb/* from a dev server.
val wvb = WebViewBundle(
    source = source,
    protocols = listOf(
        Protocol.Local(servers = mapOf("app.wvb" to "http://localhost:8080")),
    ),
)
webView.loadUrl("https://app.wvb/")
```

## Demo

The test app has a working screen: open it, tap **Open WebView Demo**
(`testapp/.../WebViewActivity.kt`). A green heading + JS-rendered origin line
confirms HTML/CSS/JS are served from the bundle.

## Notes

- Serve over **`https` with a virtual host** (e.g. `app.wvb`); avoid custom
  schemes, which get an opaque origin on Android (see above).
- Register the full host (`app.wvb`); the bundle name is its first label
  (`app`). Requests to unregistered hosts are not intercepted.
- `shouldInterceptRequest` runs off the UI thread; the suspending FFI handler is
  driven with `runBlocking` there.
