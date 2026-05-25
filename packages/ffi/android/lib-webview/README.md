# WebViewBundle for Android

System `WebView` integration for [WebViewBundle](../../../..) resources, built on
the UniFFI bindings in `:lib-android`.

It is the Android counterpart of the [`@wvb/electron`](../../../electron) and
[`wvb-tauri`](../../../tauri) packages: you register one or more protocols, each
bound to a URL scheme, and requests to those schemes are served from a bundle
source (or proxied to a local server) through
`WebViewClient.shouldInterceptRequest`.

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
    protocols = listOf(Protocol.Bundle("app")),
)

wvb.install(webView)                       // sets webView.webViewClient
webView.loadUrl("app://app.wvb/index.html") // host label "app" -> bundle "app"
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
val wvb = WebViewBundle(
    source = source,
    protocols = listOf(
        Protocol.Local("local", hosts = mapOf("myapp" to "http://localhost:8080")),
    ),
)
```

## Notes

- Use a **custom scheme** (e.g. `app`, `wvb`) rather than `http`/`https` so the
  WebView routes every request through `shouldInterceptRequest`.
- The bundle name is the first label of the host: `app://app.wvb/x` -> `app`.
- `shouldInterceptRequest` runs off the UI thread; the suspending FFI handler is
  driven with `runBlocking` there.
