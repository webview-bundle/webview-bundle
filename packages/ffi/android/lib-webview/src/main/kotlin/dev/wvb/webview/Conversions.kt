package dev.wvb.webview

import android.webkit.WebResourceResponse
import dev.wvb.HttpMethod
import dev.wvb.HttpResponse
import java.io.ByteArrayInputStream

private const val DEFAULT_MIME_TYPE = "application/octet-stream"

internal fun String?.toHttpMethod(): HttpMethod =
    when (this?.uppercase()) {
        "GET" -> HttpMethod.GET
        "HEAD" -> HttpMethod.HEAD
        "OPTIONS" -> HttpMethod.OPTIONS
        "POST" -> HttpMethod.POST
        "PUT" -> HttpMethod.PUT
        "PATCH" -> HttpMethod.PATCH
        "DELETE" -> HttpMethod.DELETE
        "TRACE" -> HttpMethod.TRACE
        "CONNECT" -> HttpMethod.CONNECT
        else -> HttpMethod.GET
    }

/**
 * Converts a WebViewBundle [HttpResponse] into a [WebResourceResponse] the
 * WebView can render.
 *
 * The `Content-Type` header is split into the MIME type and charset expected by
 * [WebResourceResponse]; the remaining headers are forwarded verbatim.
 */
internal fun HttpResponse.toWebResourceResponse(): WebResourceResponse {
    val headers = lowercasedHeaders()

    val contentType = headers["content-type"]
    val mimeType = contentType?.substringBefore(';')?.trim()?.takeIf { it.isNotEmpty() } ?: DEFAULT_MIME_TYPE
    val encoding = contentType
        ?.substringAfter("charset=", "")
        ?.substringBefore(';')
        ?.trim()
        ?.takeIf { it.isNotEmpty() }

    // Content-Type is represented via mimeType/encoding, so drop it from the
    // header map to avoid duplicating it.
    val responseHeaders = headers.filterKeys { it != "content-type" }

    val statusCode = status.toInt().coerceIn(100, 599)

    return WebResourceResponse(
        mimeType,
        encoding,
        statusCode,
        reasonPhrase(statusCode),
        responseHeaders,
        ByteArrayInputStream(body),
    )
}

private fun HttpResponse.lowercasedHeaders(): Map<String, String> =
    headers.entries.associate { (key, value) -> key.lowercase() to value }

private fun reasonPhrase(statusCode: Int): String =
    when (statusCode) {
        200 -> "OK"
        201 -> "Created"
        204 -> "No Content"
        206 -> "Partial Content"
        301 -> "Moved Permanently"
        302 -> "Found"
        304 -> "Not Modified"
        400 -> "Bad Request"
        401 -> "Unauthorized"
        403 -> "Forbidden"
        404 -> "Not Found"
        405 -> "Method Not Allowed"
        416 -> "Range Not Satisfiable"
        500 -> "Internal Server Error"
        else -> "Status $statusCode"
    }
