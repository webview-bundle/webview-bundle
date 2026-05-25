package dev.wvb.webview

/**
 * A protocol binds a URL [scheme] handled inside a [android.webkit.WebView] to a
 * WebViewBundle request handler.
 *
 * This mirrors the protocol model used by the `@wvb/electron` and `wvb-tauri`
 * packages: each protocol owns a scheme, and requests to that scheme are routed
 * to the matching handler which resolves them against the bundle source or a
 * local server.
 *
 * The bundle name is resolved from the first label of the request host, e.g.
 * `app://app.wvb/index.html` -> bundle `"app"`, path `"/index.html"`.
 *
 * Use a custom (non `http`/`https`) scheme so the WebView routes every request
 * through [android.webkit.WebViewClient.shouldInterceptRequest] instead of the
 * network stack.
 */
public sealed interface Protocol {
    public val scheme: String

    /**
     * Serves entries from the WebViewBundle [dev.wvb.BundleSource], backed by a
     * [dev.wvb.BundleUrlHandler].
     */
    public data class Bundle(override val scheme: String) : Protocol

    /**
     * Proxies requests to local HTTP servers, backed by a
     * [dev.wvb.LocalUrlHandler]. [hosts] maps a virtual host to a local base URL,
     * e.g. `{"myapp" to "http://localhost:8080"}`.
     */
    public data class Local(
        override val scheme: String,
        val hosts: Map<String, String>,
    ) : Protocol
}
