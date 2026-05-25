package dev.wvb.webview

import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import dev.wvb.BundleSource
import dev.wvb.BundleUrlHandler
import dev.wvb.HttpMethod
import dev.wvb.HttpResponse
import dev.wvb.LocalUrlHandler
import dev.wvb.Remote
import dev.wvb.Updater
import kotlinx.coroutines.runBlocking
import java.io.ByteArrayInputStream

private typealias RequestHandler = suspend (HttpMethod, String, Map<String, String>?) -> HttpResponse

/**
 * Serves WebViewBundle resources to a system [WebView].
 *
 * `WebViewBundle` is the Android counterpart to the `@wvb/electron` and
 * `wvb-tauri` packages. It wires one or more [Protocol]s to the WebView's
 * resource loader: requests to a registered host (over `https`/`http`) are
 * resolved from the bundle [source] (or proxied to a local server) instead of
 * hitting the network; everything else falls through unchanged.
 *
 * Android matches on **host over `https`** rather than a custom scheme so the
 * served page keeps a real secure origin (see [Protocol]).
 *
 * ```kotlin
 * val wvb = WebViewBundle(
 *     source = BundleSource(
 *         BundleSourceConfig(
 *             builtinDir = builtinDir.absolutePath,
 *             remoteDir = remoteDir.absolutePath,
 *             builtinManifestFilepath = null,
 *             remoteManifestFilepath = null,
 *         )
 *     ),
 *     protocols = listOf(Protocol.Bundle("app.wvb")),
 * )
 * webView.webViewClient = wvb.createWebViewClient()
 * webView.loadUrl("https://app.wvb/index.html") // host label "app" -> bundle "app"
 * ```
 *
 * @param source the bundle source requests are served from.
 * @param protocols the protocols to register; each host must be unique.
 * @param remote optional remote client, exposed for update flows.
 * @param updater optional updater, exposed for update flows.
 * @param onError invoked when a request handler throws; the request then yields
 *   a `500` response.
 */
public class WebViewBundle(
    public val source: BundleSource,
    protocols: List<Protocol>,
    public val remote: Remote? = null,
    public val updater: Updater? = null,
    private val onError: ((Throwable) -> Unit)? = null,
) {
    private val handlersByHost: Map<String, RequestHandler> = buildHandlers(source, protocols)

    /** The hosts this instance intercepts. */
    public val hosts: Set<String> get() = handlersByHost.keys

    /**
     * Resolves [request] against the registered protocols.
     *
     * Returns a [WebResourceResponse] when the request targets a registered host
     * over `https`/`http`, or `null` to let the WebView load the request
     * normally. Call this from your own
     * [android.webkit.WebViewClient.shouldInterceptRequest] override, or use
     * [createWebViewClient] / [install].
     */
    public fun intercept(request: WebResourceRequest): WebResourceResponse? {
        val url = request.url
        val scheme = url.scheme?.lowercase()
        if (scheme != "https" && scheme != "http") return null
        val host = url.host?.lowercase() ?: return null
        val handler = handlersByHost[host] ?: return null

        val method = request.method.toHttpMethod()
        val uri = url.toString()
        val headers = request.requestHeaders.takeIf { it.isNotEmpty() }

        return try {
            // shouldInterceptRequest runs off the UI thread, so blocking on the
            // suspending handler here is safe.
            runBlocking { handler(method, uri, headers) }.toWebResourceResponse()
        } catch (e: Throwable) {
            onError?.invoke(e)
            errorResponse(e)
        }
    }

    /** Creates a [WebViewClient] that intercepts the registered hosts. */
    public fun createWebViewClient(): WebViewBundleClient = WebViewBundleClient(this)

    /** Convenience for `webView.webViewClient = createWebViewClient()`. */
    public fun install(webView: WebView) {
        webView.webViewClient = createWebViewClient()
    }

    private companion object {
        fun buildHandlers(source: BundleSource, protocols: List<Protocol>): Map<String, RequestHandler> {
            val handlers = LinkedHashMap<String, RequestHandler>()
            for (protocol in protocols) {
                val handlerFn: RequestHandler = when (protocol) {
                    is Protocol.Bundle -> {
                        val urlHandler = BundleUrlHandler(source)
                        val fn: RequestHandler =
                            { method, uri, headers -> urlHandler.handle(method, uri, headers) }
                        fn
                    }

                    is Protocol.Local -> {
                        val urlHandler = LocalUrlHandler(protocol.servers)
                        val fn: RequestHandler =
                            { method, uri, headers -> urlHandler.handle(method, uri, headers) }
                        fn
                    }
                }
                for (host in protocol.hosts) {
                    val key = host.lowercase()
                    require(key.isNotEmpty()) { "protocol host must not be empty" }
                    require(key !in handlers) { "duplicate protocol host: $key" }
                    handlers[key] = handlerFn
                }
            }
            return handlers
        }

        fun errorResponse(e: Throwable): WebResourceResponse {
            val message = (e.message ?: e.javaClass.simpleName).toByteArray()
            return WebResourceResponse(
                "text/plain",
                "utf-8",
                500,
                "Internal Server Error",
                emptyMap(),
                ByteArrayInputStream(message),
            )
        }
    }
}
