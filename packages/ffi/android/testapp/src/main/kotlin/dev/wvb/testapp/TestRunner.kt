package dev.wvb.testapp

import android.content.Context
import dev.wvb.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

data class TestResult(
    val name: String,
    val passed: Boolean,
    val error: String? = null,
)

class TestRunner(private val context: Context) {
    private val indexHtml = "<!DOCTYPE html>".toByteArray()
    private val indexJs = "console.log('hello')".toByteArray()

    suspend fun run(): List<TestResult> = withContext(Dispatchers.IO) {
        val results = mutableListOf<TestResult>()

        fun test(name: String, block: () -> Unit) {
            try {
                block()
                results.add(TestResult(name, true))
            } catch (e: Throwable) {
                results.add(TestResult(name, false, e.message ?: e.javaClass.simpleName))
            }
        }

        suspend fun testSuspend(name: String, block: suspend () -> Unit) {
            try {
                block()
                results.add(TestResult(name, true))
            } catch (e: Throwable) {
                results.add(TestResult(name, false, e.message ?: e.javaClass.simpleName))
            }
        }

        // ── HttpResponse ─────────────────────────────────────────────────
        test("HttpResponse: init") {
            val response = HttpResponse(
                status = 200u,
                headers = mapOf("Content-Type" to "text/html"),
                body = byteArrayOf()
            )
            check(response.status == 200.toUShort()) { "status mismatch" }
            check(response.headers["Content-Type"] == "text/html") { "header mismatch" }
        }

        // ── BundleBuilder ────────────────────────────────────────────────
        test("BundleBuilder: build") {
            val builder = BundleBuilder(null)
            check(builder.version() == Version.V1)
            check(builder.entryPaths().isEmpty())
            check(!builder.insertEntry("/index.js", indexJs, null, null)) { "first insert should return false" }
            check(!builder.insertEntry("/index.html", indexHtml, null, null))
            check(builder.insertEntry("/index.html", indexHtml, null, null)) { "duplicate insert should return true" }
            check(builder.entryPaths().size == 2)
            check(builder.containsEntry("/index.js"))
            check(builder.containsEntry("/index.html"))
            check(!builder.containsEntry("/not_exists"))
            builder.build(null)
        }

        test("BundleBuilder: with options") {
            val builder = BundleBuilder(Version.V1)
            builder.insertEntry("/index.js", indexJs, null, null)
            builder.build(
                BuildOptions(
                    header = BuildHeaderOptions(checksum = ChecksumWriteOptions(seed = 1u)),
                    index = BuildIndexOptions(checksum = ChecksumWriteOptions(seed = 2u)),
                    dataChecksum = ChecksumWriteOptions(seed = 3u),
                )
            )
        }

        test("BundleBuilder: removeEntry") {
            val builder = BundleBuilder(null)
            builder.insertEntry("/index.js", indexJs, null, null)
            check(builder.removeEntry("/index.js"))
            check(!builder.containsEntry("/index.js"))
            check(!builder.removeEntry("/index.js"))
        }

        // ── Bundle ───────────────────────────────────────────────────────
        test("Bundle: version") {
            val bundle = BundleBuilder(Version.V1).build(null)
            check(bundle.descriptor().header().version() == Version.V1)
        }

        test("Bundle: getData") {
            val builder = BundleBuilder(null)
            builder.insertEntry("/index.js", indexJs, null, null)
            builder.insertEntry("/index.html", indexHtml, null, null)
            val bundle = builder.build(null)
            check(bundle.getData("/index.js").contentEquals(indexJs))
            check(bundle.getData("/index.html").contentEquals(indexHtml))
            check(bundle.getData("/not_exists") == null)
        }

        test("Bundle: index entries") {
            val builder = BundleBuilder(null)
            builder.insertEntry("/index.js", indexJs, null, null)
            builder.insertEntry("/index.html", indexHtml, null, null)
            val bundle = builder.build(null)
            val index = bundle.descriptor().index()
            check(index.entries().size == 2)
            check(index.containsPath("/index.js"))
            check(index.containsPath("/index.html"))
            check(!index.containsPath("/not_exists"))
            check(index.getEntry("/index.js") != null)
            check(index.getEntry("/not_exists") == null)
        }

        test("Bundle: index entry metadata") {
            val builder = BundleBuilder(null)
            builder.insertEntry("/index.js", indexJs, "text/javascript", mapOf("x-custom" to "value"))
            val bundle = builder.build(null)
            val entry = bundle.descriptor().index().getEntry("/index.js")
            checkNotNull(entry)
            check(entry.contentType == "text/javascript") { "contentType=${entry.contentType}" }
            check(entry.contentLength == indexJs.size.toULong())
            check(entry.headers["x-custom"] == "value")
        }

        test("Bundle: descriptor indexEntries") {
            val builder = BundleBuilder(null)
            builder.insertEntry("/index.js", indexJs, null, null)
            val bundle = builder.build(null)
            val descriptor = bundle.descriptor()
            check(descriptor.indexEntries().size == 1)
            check(descriptor.containsPath("/index.js"))
            check(!descriptor.containsPath("/not_exists"))
            check(descriptor.getIndexEntry("/index.js") != null)
        }

        test("Bundle: getDataChecksum") {
            val builder = BundleBuilder(null)
            builder.insertEntry("/index.js", indexJs, null, null)
            val bundle = builder.build(null)
            check(bundle.getDataChecksum("/index.js") != null)
            check(bundle.getDataChecksum("/not_exists") == null)
        }

        // ── Read/Write ───────────────────────────────────────────────────
        test("Read/Write: bytes roundtrip") {
            val builder = BundleBuilder(null)
            builder.insertEntry("/index.js", indexJs, null, null)
            builder.insertEntry("/index.html", indexHtml, null, null)
            val bundle = builder.build(null)

            val bytes = writeBundleToBytes(bundle)
            check(bytes.isNotEmpty())

            val loaded = readBundleFromBytes(bytes)
            check(loaded.descriptor().header().version() == Version.V1)
            check(loaded.getData("/index.js").contentEquals(indexJs))
            check(loaded.getData("/index.html").contentEquals(indexHtml))
        }

        testSuspend("Read/Write: file roundtrip") {
            val tmpFile = File(context.filesDir, "test_bundle.wvb")
            try {
                val builder = BundleBuilder(null)
                builder.insertEntry("/index.js", indexJs, null, null)
                builder.insertEntry("/index.html", indexHtml, null, null)
                val bundle = builder.build(null)

                writeBundle(bundle, tmpFile.absolutePath)
                check(tmpFile.exists() && tmpFile.length() > 0)

                val loaded = readBundle(tmpFile.absolutePath)
                check(loaded.descriptor().header().version() == Version.V1)
                val index = loaded.descriptor().index()
                check(index.entries().size == 2)
                check(index.containsPath("/index.js"))
                check(loaded.getData("/index.js").contentEquals(indexJs))
            } finally {
                tmpFile.delete()
            }
        }

        // ── Protocol ─────────────────────────────────────────────────────
        testSuspend("BundleProtocolHandler: 200 index.html") {
            withSource { source ->
                val handler = BundleProtocolHandler(source)
                val response = handler.handle(HttpMethod.GET, "https://app.wvb/index.html", null)
                check(response.status == 200.toUShort()) { "status=${response.status}" }
                check(response.body.isNotEmpty()) { "body should not be empty" }
                check(response.headers.containsKey("content-type")) { "content-type header missing" }
            }
        }

        testSuspend("BundleProtocolHandler: 200 root redirect") {
            withSource { source ->
                val handler = BundleProtocolHandler(source)
                val response = handler.handle(HttpMethod.GET, "https://app.wvb/", null)
                check(response.status == 200.toUShort()) { "status=${response.status}" }
            }
        }

        testSuspend("BundleProtocolHandler: 404 not found") {
            withSource { source ->
                val handler = BundleProtocolHandler(source)
                val response = handler.handle(HttpMethod.GET, "https://app.wvb/not_found.html", null)
                check(response.status == 404.toUShort()) { "status=${response.status}" }
            }
        }

        testSuspend("BundleProtocolHandler: HEAD 200") {
            withSource { source ->
                val handler = BundleProtocolHandler(source)
                val response = handler.handle(HttpMethod.HEAD, "https://app.wvb/index.html", null)
                check(response.status == 200.toUShort()) { "status=${response.status}" }
                check(response.body.isEmpty()) { "HEAD response body must be empty" }
            }
        }

        testSuspend("BundleProtocolHandler: exact path resolver does not rewrite to index.html") {
            withSource { source ->
                val options = BundleProtocolOptions(pathResolver = PathResolver.EXACT)
                val handler = BundleProtocolHandler(source, options)
                check(handler.handle(HttpMethod.GET, "https://app.wvb/index.html", null).status == 200.toUShort())
                val response = handler.handle(HttpMethod.GET, "https://app.wvb/", null)
                check(response.status == 404.toUShort()) { "status=${response.status}" }
            }
        }

        testSuspend("BundleProtocolHandler: allowWvbSuffixOnly rejects other hosts") {
            withSource { source ->
                val options = BundleProtocolOptions(
                    bundleResolver = BundleResolver.Hostname(
                        segment = HostnameSegment.First,
                        allowWvbSuffixOnly = true,
                    ),
                )
                val handler = BundleProtocolHandler(source, options)
                check(handler.handle(HttpMethod.GET, "https://app.wvb/index.html", null).status == 200.toUShort())
                // Recorded outside the catch, which would swallow an `error(...)` raised inside it.
                var rejected = false
                try {
                    handler.handle(HttpMethod.GET, "https://app.example.com/index.html", null)
                } catch (e: Exception) {
                    // expected: the bundle name is not resolved, so no bundle is found
                    rejected = true
                }
                check(rejected) { "expected an error for a host without the .wvb suffix" }
            }
        }

        test("ProxyProtocolHandler: init") {
            ProxyProtocolHandler(mapOf("myapp" to "http://localhost:9999"))
        }

        testSuspend("BundleProtocolHandler: a request body is accepted") {
            withSource { source ->
                val handler = BundleProtocolHandler(source)
                // The bundle protocol serves GET/HEAD only, but the body still travels over the FFI.
                val response = handler.handle(
                    HttpMethod.POST,
                    "https://app.wvb/index.html",
                    null,
                    """{"hello":"world"}""".toByteArray(),
                )
                check(response.status == 405.toUShort()) { "status=${response.status}" }
            }
        }

        testSuspend("ProxyProtocolHandler: unknown host error") {
            val handler = ProxyProtocolHandler(mapOf("known" to "http://localhost:9999"))
            var rejected = false
            try {
                handler.handle(HttpMethod.GET, "https://unknown.wvb/index.html", null)
            } catch (e: Exception) {
                // expected: no proxy target for "unknown.wvb"
                rejected = true
            }
            check(rejected) { "expected an error for an unknown host" }
        }

        testSuspend("ProxyProtocolHandler: custom resolver receives the uri") {
            var seen: String? = null
            val resolver = object : ProxyResolver {
                override suspend fun resolve(uri: String): String? {
                    seen = uri
                    return null // do not proxy
                }
            }
            val handler = ProxyProtocolHandler.custom(resolver)
            var rejected = false
            try {
                handler.handle(HttpMethod.GET, "https://app.wvb/index.html", null)
            } catch (e: Exception) {
                // expected: the target is unresolved
                rejected = true
            }
            check(rejected) { "expected an error when the resolver returns null" }
            check(seen == "https://app.wvb/index.html") { "resolver saw uri=$seen" }
        }

        results
    }

    private fun setupFixtures(): File {
        val remoteDir = File(context.filesDir, "fixture_remote")
        if (!File(remoteDir, "manifest.json").exists()) {
            remoteDir.mkdirs()
            context.assets.open("fixtures/remote/manifest.json").use { input ->
                File(remoteDir, "manifest.json").outputStream().use { input.copyTo(it) }
            }
            val appDir = File(remoteDir, "app")
            appDir.mkdirs()
            context.assets.open("fixtures/remote/app/1.0.0.wvb").use { input ->
                File(appDir, "1.0.0.wvb").outputStream().use { input.copyTo(it) }
            }
        }
        return remoteDir
    }

    private suspend fun withSource(block: suspend (Source) -> Unit) {
        val remoteDir = setupFixtures()
        val source = Source(
            SourceConfig(
                builtinDir = context.cacheDir.absolutePath,
                remoteDir = remoteDir.absolutePath,
                builtinManifestFilepath = null,
                remoteManifestFilepath = null,
            )
        )
        block(source)
    }
}
