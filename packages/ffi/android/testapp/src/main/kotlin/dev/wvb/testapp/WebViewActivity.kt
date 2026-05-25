package dev.wvb.testapp

import android.os.Bundle
import android.webkit.WebView
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.lifecycleScope
import dev.wvb.BundleBuilder
import dev.wvb.BundleSource
import dev.wvb.BundleSourceConfig
import dev.wvb.webview.Protocol
import dev.wvb.webview.WebViewBundle
import dev.wvb.writeBundleToBytes
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

/**
 * Minimal manual check for the WebView integration: builds a bundle in memory,
 * serves it through [WebViewBundle], and loads it in a system [WebView].
 *
 * A successful screen shows a green "WebViewBundle" heading (CSS applied) and a
 * status line printed by JS that includes `https://app.wvb` as the origin —
 * proving HTML, CSS and JS sub-resources are all served from the bundle over a
 * real secure origin.
 */
class WebViewActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val webView = WebView(this)
        webView.settings.javaScriptEnabled = true
        webView.settings.domStorageEnabled = true
        setContentView(webView)

        lifecycleScope.launch {
            val source = withContext(Dispatchers.IO) { setupSource() }
            val wvb = WebViewBundle(source, listOf(Protocol.Bundle("app.wvb")))
            wvb.install(webView)
            webView.loadUrl("https://app.wvb/index.html")
        }
    }

    private fun setupSource(): BundleSource {
        val dir = File(cacheDir, "webview-demo")
        val remoteDir = File(dir, "remote")
        val appDir = File(remoteDir, "app").apply { mkdirs() }
        val builtinDir = File(dir, "builtin").apply { mkdirs() }

        File(remoteDir, "manifest.json").writeText(
            """{"manifestVersion":1,"entries":{"app":{"versions":{"1.0.0":{}},"currentVersion":"1.0.0"}}}"""
        )

        val builder = BundleBuilder(null)
        builder.insertEntry("/index.html", INDEX_HTML.toByteArray(), "text/html", null)
        builder.insertEntry("/style.css", STYLE_CSS.toByteArray(), "text/css", null)
        builder.insertEntry("/app.js", APP_JS.toByteArray(), "text/javascript", null)
        val bundle = builder.build(null)
        File(appDir, "app_1.0.0.wvb").writeBytes(writeBundleToBytes(bundle))

        return BundleSource(
            BundleSourceConfig(
                builtinDir = builtinDir.absolutePath,
                remoteDir = remoteDir.absolutePath,
                builtinManifestFilepath = null,
                remoteManifestFilepath = null,
            )
        )
    }

    private companion object {
        const val INDEX_HTML = """<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>WebViewBundle</title>
<link rel="stylesheet" href="/style.css">
</head>
<body>
<h1 id="title">WebViewBundle</h1>
<p id="status">checking…</p>
<script src="/app.js"></script>
</body>
</html>
"""

        const val STYLE_CSS = """
body { font-family: sans-serif; padding: 24px; }
#title { color: #2E7D32; }
#status.ok { color: #2E7D32; font-weight: bold; }
"""

        const val APP_JS = """
var el = document.getElementById('status');
el.textContent = 'OK · served from ' + location.origin;
el.className = 'ok';
"""
    }
}
