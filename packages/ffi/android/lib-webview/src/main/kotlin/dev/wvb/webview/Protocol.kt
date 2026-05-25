package dev.wvb.webview

/**
 * A protocol binds one or more virtual **hosts** to a WebViewBundle request
 * handler. Requests to those hosts over `https` (or `http`) are intercepted via
 * [android.webkit.WebViewClient.shouldInterceptRequest] and served from the
 * bundle source (or proxied to a local server).
 *
 * Unlike the desktop `@wvb/electron` / `wvb-tauri` packages, Android matches on
 * **host over `https`**, not on a custom scheme: a custom scheme would give the
 * page an opaque (`"null"`) origin, breaking `localStorage`, `fetch`, cookies
 * and Service Workers. Serving over a virtual `https` host (the approach used by
 * `WebViewAssetLoader`) keeps a real secure origin. Requests whose host is not
 * registered fall through to the network unchanged.
 *
 * The bundle name is the first label of the host, e.g. `https://app.wvb/x`
 * resolves to bundle `"app"`.
 */
public sealed interface Protocol {
    /** The request hosts this protocol intercepts (e.g. `"app.wvb"`). */
    public val hosts: Set<String>

    /** Serves bundle entries for the given virtual [hosts]. */
    public data class Bundle(override val hosts: Set<String>) : Protocol {
        public constructor(vararg hosts: String) : this(hosts.toSet())
    }

    /**
     * Proxies requests to local HTTP servers. Each key of [servers] is a virtual
     * host to intercept; its value is the local base URL, e.g.
     * `{"app.wvb" to "http://localhost:8080"}`.
     */
    public data class Local(val servers: Map<String, String>) : Protocol {
        override val hosts: Set<String> get() = servers.keys
    }
}
