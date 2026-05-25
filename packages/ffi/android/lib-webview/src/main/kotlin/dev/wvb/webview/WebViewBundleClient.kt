package dev.wvb.webview

import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient

/**
 * A [WebViewClient] that serves WebViewBundle resources for the schemes
 * registered on [webViewBundle].
 *
 * Subclass this to add custom navigation handling while keeping bundle
 * interception; remember to call `super.shouldInterceptRequest(...)` and return
 * its result when non-null:
 *
 * ```kotlin
 * class MyClient(wvb: WebViewBundle) : WebViewBundleClient(wvb) {
 *     override fun shouldInterceptRequest(
 *         view: WebView,
 *         request: WebResourceRequest,
 *     ): WebResourceResponse? =
 *         super.shouldInterceptRequest(view, request) ?: customResource(request)
 * }
 * ```
 */
public open class WebViewBundleClient(
    private val webViewBundle: WebViewBundle,
) : WebViewClient() {
    override fun shouldInterceptRequest(
        view: WebView,
        request: WebResourceRequest,
    ): WebResourceResponse? = webViewBundle.intercept(request)
}
