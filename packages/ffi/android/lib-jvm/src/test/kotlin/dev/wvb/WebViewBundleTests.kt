package dev.wvb

import com.sun.net.httpserver.HttpExchange
import com.sun.net.httpserver.HttpServer
import java.net.InetSocketAddress
import java.util.concurrent.Executors
import kotlinx.coroutines.runBlocking
import kotlin.io.path.createTempDirectory
import kotlin.io.path.deleteRecursively
import kotlin.test.*

class WebViewBundleTests {
    private val indexHtml = "<!DOCTYPE html>".toByteArray()
    private val indexJs = "console.log('hello')".toByteArray()

    // ── HttpResponse ─────────────────────────────────────────────────────────

    @Test
    fun httpResponseInit() {
        val response = HttpResponse(
            status = 200u,
            headers = mapOf("Content-Type" to "text/html"),
            body = byteArrayOf()
        )
        assertEquals(200.toUShort(), response.status)
        assertEquals("text/html", response.headers["Content-Type"])
        assertEquals(0, response.body.size)
    }

    // ── BundleBuilder ────────────────────────────────────────────────────────

    @Test
    fun buildBundle() {
        val builder = BundleBuilder(null)
        assertEquals(Version.V1, builder.version())
        assertTrue(builder.entryPaths().isEmpty())

        assertFalse(builder.insertEntry("/index.js", indexJs, null, null))
        assertFalse(builder.insertEntry("/index.html", indexHtml, null, null))
        assertTrue(builder.insertEntry("/index.html", indexHtml, null, null)) // same path → replaced

        assertEquals(2, builder.entryPaths().size)
        assertTrue(builder.containsEntry("/index.js"))
        assertTrue(builder.containsEntry("/index.html"))
        assertFalse(builder.containsEntry("/not_exists"))

        assertNotNull(builder.build(null))
    }

    @Test
    fun buildBundleWithVersion() {
        val builder = BundleBuilder(Version.V1)
        assertEquals(Version.V1, builder.version())
        assertNotNull(builder.build(null))
    }

    @Test
    fun buildBundleWithOptions() {
        val builder = BundleBuilder(null)
        builder.insertEntry("/index.js", indexJs, null, null)
        assertNotNull(
            builder.build(
                BuildOptions(
                    header = BuildHeaderOptions(checksumSeed = 1u),
                    index = BuildIndexOptions(checksumSeed = 2u),
                    dataChecksumSeed = 3u,
                )
            )
        )
    }

    @Test
    fun removeEntry() {
        val builder = BundleBuilder(null)
        builder.insertEntry("/index.js", indexJs, null, null)
        assertTrue(builder.containsEntry("/index.js"))
        assertTrue(builder.removeEntry("/index.js"))
        assertFalse(builder.containsEntry("/index.js"))
        assertFalse(builder.removeEntry("/index.js")) // already gone
    }

    // ── Bundle ───────────────────────────────────────────────────────────────

    @Test
    fun bundleVersion() {
        val builder = BundleBuilder(Version.V1)
        val bundle = builder.build(null)
        assertEquals(Version.V1, bundle.descriptor().header().version())
    }

    @Test
    fun bundleGetData() {
        val builder = BundleBuilder(null)
        builder.insertEntry("/index.js", indexJs, null, null)
        builder.insertEntry("/index.html", indexHtml, null, null)
        val bundle = builder.build(null)

        assertContentEquals(indexJs, bundle.getData("/index.js"))
        assertContentEquals(indexHtml, bundle.getData("/index.html"))
        assertNull(bundle.getData("/not_exists"))
    }

    @Test
    fun bundleIndexEntries() {
        val builder = BundleBuilder(null)
        builder.insertEntry("/index.js", indexJs, null, null)
        builder.insertEntry("/index.html", indexHtml, null, null)
        val bundle = builder.build(null)
        val index = bundle.descriptor().index()

        assertEquals(2, index.entries().size)
        assertTrue(index.containsPath("/index.js"))
        assertTrue(index.containsPath("/index.html"))
        assertFalse(index.containsPath("/not_exists"))
        assertNotNull(index.getEntry("/index.js"))
        assertNull(index.getEntry("/not_exists"))
    }

    @Test
    fun bundleIndexEntryMetadata() {
        val builder = BundleBuilder(null)
        builder.insertEntry("/index.js", indexJs, "text/javascript", mapOf("x-custom" to "value"))
        val bundle = builder.build(null)
        val entry = bundle.descriptor().index().getEntry("/index.js")

        assertNotNull(entry)
        assertEquals("text/javascript", entry.contentType)
        assertEquals(indexJs.size.toULong(), entry.contentLength)
        assertEquals("value", entry.headers["x-custom"])
    }

    @Test
    fun bundleDescriptorIndexEntries() {
        val builder = BundleBuilder(null)
        builder.insertEntry("/index.js", indexJs, null, null)
        val bundle = builder.build(null)
        val descriptor = bundle.descriptor()
        val entries = descriptor.indexEntries()

        assertEquals(1, entries.size)
        assertTrue(descriptor.containsPath("/index.js"))
        assertFalse(descriptor.containsPath("/not_exists"))
        assertNotNull(descriptor.getIndexEntry("/index.js"))
        assertNull(descriptor.getIndexEntry("/not_exists"))
    }

    @Test
    fun bundleGetDataChecksum() {
        val builder = BundleBuilder(null)
        builder.insertEntry("/index.js", indexJs, null, null)
        val bundle = builder.build(null)

        assertNotNull(bundle.getDataChecksum("/index.js"))
        assertNull(bundle.getDataChecksum("/not_exists"))
    }

    // ── Read/Write ───────────────────────────────────────────────────────────

    @Test
    fun readWriteBundleBytes() {
        val builder = BundleBuilder(null)
        builder.insertEntry("/index.js", indexJs, null, null)
        builder.insertEntry("/index.html", indexHtml, null, null)
        val bundle = builder.build(null)

        val bytes = writeBundleToBytes(bundle)
        assertTrue(bytes.isNotEmpty())

        val loaded = readBundleFromBytes(bytes)
        assertEquals(Version.V1, loaded.descriptor().header().version())
        assertContentEquals(indexJs, loaded.getData("/index.js"))
        assertContentEquals(indexHtml, loaded.getData("/index.html"))
    }

    @Test
    @OptIn(kotlin.io.path.ExperimentalPathApi::class)
    fun readWriteBundleFile(): Unit = runBlocking {
        val tmpDir = createTempDirectory("wvb-test")
        try {
            val builder = BundleBuilder(null)
            builder.insertEntry("/index.js", indexJs, null, null)
            builder.insertEntry("/index.html", indexHtml, null, null)
            val bundle = builder.build(null)

            val filePath = tmpDir.resolve("bundle.wvb").toString()
            writeBundle(bundle, filePath)

            val loaded = readBundle(filePath)
            assertEquals(Version.V1, loaded.descriptor().header().version())
            val index = loaded.descriptor().index()
            assertEquals(2, index.entries().size)
            assertTrue(index.containsPath("/index.js"))
            assertTrue(index.containsPath("/index.html"))
            assertContentEquals(indexJs, loaded.getData("/index.js"))
        } finally {
            tmpDir.deleteRecursively()
        }
    }

    // ── Remote ───────────────────────────────────────────────────────────────

    @Test
    fun remoteListBundles(): Unit = runBlocking {
        withMockServer { port ->
            val remote = Remote("http://localhost:$port")
            val bundles = remote.listBundles(null)
            assertEquals(1, bundles.size)
            assertEquals("bundle1", bundles[0].name)
            assertEquals("1.0.0", bundles[0].version)
        }
    }

    @Test
    fun remoteGetInfo(): Unit = runBlocking {
        withMockServer { port ->
            val remote = Remote("http://localhost:$port")
            val info = remote.getInfo("bundle1", null)
            assertEquals("bundle1", info.name)
            assertEquals("1.0.0", info.version)
        }
    }

    @Test
    fun remoteDownload(): Unit = runBlocking {
        withMockServer { port ->
            val remote = Remote("http://localhost:$port")
            val result = remote.download("bundle1", null)
            assertEquals("bundle1", result.info.name)
            assertEquals("1.0.0", result.info.version)
            assertContentEquals(indexHtml, result.bundle.getData("/index.html"))
        }
    }

    @Test
    fun remoteDownloadVersion(): Unit = runBlocking {
        withMockServer { port ->
            val remote = Remote("http://localhost:$port")
            val result = remote.downloadVersion("bundle1", "1.0.0")
            assertEquals("bundle1", result.info.name)
            assertEquals("1.0.0", result.info.version)
            assertContentEquals(indexHtml, result.bundle.getData("/index.html"))
        }
    }

    @Test
    fun remoteDownloadVersionForbidden(): Unit = runBlocking {
        withMockServer(allowOnlyLatest = true) { port ->
            val remote = Remote("http://localhost:$port")
            val ex = assertFailsWith<Exception> {
                remote.downloadVersion("bundle1", "1.0.0")
            }
            assertTrue(
                ex.message?.contains("remote forbidden", ignoreCase = true) == true,
                "Expected 'remote forbidden' in error message, got: ${ex.message}"
            )
        }
    }

    @Test
    fun remoteBundleNotFound(): Unit = runBlocking {
        withMockServer { port ->
            val remote = Remote("http://localhost:$port")
            val ex = assertFailsWith<Exception> {
                remote.download("not_found", null)
            }
            assertTrue(
                ex.message?.contains("bundle not found", ignoreCase = true) == true,
                "Expected 'bundle not found' in error message, got: ${ex.message}"
            )
        }
    }

    // ── Mock server helpers ───────────────────────────────────────────────────

    private fun makeBundleBytes(): ByteArray {
        val builder = BundleBuilder(null)
        builder.insertEntry("/index.html", indexHtml, "text/html", null)
        return writeBundleToBytes(builder.build(null))
    }

    private fun sendBundleResponse(
        exchange: HttpExchange,
        bundleName: String,
        version: String,
        bytes: ByteArray,
    ) {
        exchange.responseHeaders.apply {
            set("Content-Type", "application/webview-bundle")
            set("webview-bundle-name", bundleName)
            set("webview-bundle-version", version)
        }
        if (exchange.requestMethod.equals("HEAD", ignoreCase = true)) {
            exchange.sendResponseHeaders(200, -1)
            exchange.responseBody.close()
        } else {
            exchange.sendResponseHeaders(200, bytes.size.toLong())
            exchange.responseBody.use { it.write(bytes) }
        }
    }

    private fun startMockServer(allowOnlyLatest: Boolean = false): Pair<HttpServer, Int> {
        val server = HttpServer.create(InetSocketAddress(0), 0)
        server.executor = Executors.newFixedThreadPool(4)
        val port = server.address.port
        val bundleBytes = makeBundleBytes()

        server.createContext("/bundles") { exchange ->
            val segments = exchange.requestURI.path
                .trimStart('/')
                .split("/")
                .filter { it.isNotEmpty() }

            when (segments.size) {
                1 -> {
                    // GET /bundles → list
                    val json = """[{"name":"bundle1","version":"1.0.0"}]""".toByteArray()
                    exchange.responseHeaders.set("Content-Type", "application/json")
                    exchange.sendResponseHeaders(200, json.size.toLong())
                    exchange.responseBody.use { it.write(json) }
                }
                2 -> {
                    // HEAD or GET /bundles/{name}
                    val name = segments[1]
                    if (name == "bundle1") {
                        sendBundleResponse(exchange, name, "1.0.0", bundleBytes)
                    } else {
                        exchange.sendResponseHeaders(404, -1)
                        exchange.responseBody.close()
                    }
                }
                3 -> {
                    // GET /bundles/{name}/{version}
                    val name = segments[1]
                    val ver = segments[2]
                    when {
                        allowOnlyLatest -> {
                            exchange.sendResponseHeaders(403, -1)
                            exchange.responseBody.close()
                        }
                        name == "bundle1" && ver == "1.0.0" -> {
                            sendBundleResponse(exchange, name, ver, bundleBytes)
                        }
                        else -> {
                            exchange.sendResponseHeaders(404, -1)
                            exchange.responseBody.close()
                        }
                    }
                }
                else -> {
                    exchange.sendResponseHeaders(404, -1)
                    exchange.responseBody.close()
                }
            }
        }

        server.start()
        return Pair(server, port)
    }

    private suspend fun withMockServer(
        allowOnlyLatest: Boolean = false,
        block: suspend (port: Int) -> Unit,
    ) {
        val (server, port) = startMockServer(allowOnlyLatest)
        try {
            block(port)
        } finally {
            server.stop(0)
        }
    }
}
