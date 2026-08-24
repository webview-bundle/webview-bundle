import Foundation
import WebViewBundleLibrary

struct TestResult: Identifiable {
    let id = UUID()
    let name: String
    let passed: Bool
    let error: String?
}

@MainActor
final class TestRunner: ObservableObject {
    @Published var results: [TestResult] = []
    @Published var isRunning = false

    private let indexHtml = Data("<!DOCTYPE html>".utf8)
    private let indexJs = Data("console.log('hello')".utf8)

    func run() async {
        isRunning = true
        results = []

        var newResults: [TestResult] = []

        func test(_ name: String, _ block: () throws -> Void) {
            do {
                try block()
                newResults.append(TestResult(name: name, passed: true, error: nil))
            } catch {
                newResults.append(TestResult(name: name, passed: false, error: error.localizedDescription))
            }
        }

        func testAsync(_ name: String, _ block: () async throws -> Void) async {
            do {
                try await block()
                newResults.append(TestResult(name: name, passed: true, error: nil))
            } catch {
                newResults.append(TestResult(name: name, passed: false, error: error.localizedDescription))
            }
        }

        // ── HttpResponse ─────────────────────────────────────────────────
        test("HttpResponse: init") {
            let response = HttpResponse(status: 200, headers: ["Content-Type": "text/html"], body: Data())
            guard response.status == 200 else { throw Fail("status mismatch") }
            guard response.headers["Content-Type"] == "text/html" else { throw Fail("header mismatch") }
        }

        // ── BundleBuilder ────────────────────────────────────────────────
        test("BundleBuilder: build") {
            let builder = BundleBuilder(version: nil)
            guard builder.version() == .v1 else { throw Fail("version mismatch") }
            guard builder.entryPaths().isEmpty else { throw Fail("expected empty entryPaths") }
            let ins1 = try builder.insertEntry(path: "/index.js", data: indexJs, contentType: nil, headers: nil)
            guard ins1 == false else { throw Fail("first insert should return false") }
            let ins2 = try builder.insertEntry(path: "/index.html", data: indexHtml, contentType: nil, headers: nil)
            guard ins2 == false else { throw Fail("second insert should return false") }
            let ins3 = try builder.insertEntry(path: "/index.html", data: indexHtml, contentType: nil, headers: nil)
            guard ins3 == true else { throw Fail("duplicate insert should return true") }
            guard builder.entryPaths().count == 2 else { throw Fail("expected 2 entries") }
            guard builder.containsEntry(path: "/index.js") else { throw Fail("missing /index.js") }
            guard builder.containsEntry(path: "/index.html") else { throw Fail("missing /index.html") }
            guard !builder.containsEntry(path: "/not_exists") else { throw Fail("/not_exists should not exist") }
            _ = try builder.build(options: nil)
        }

        test("BundleBuilder: with options") {
            let builder = BundleBuilder(version: .v1)
            _ = try builder.insertEntry(path: "/index.js", data: indexJs, contentType: nil, headers: nil)
            _ = try builder.build(options: BuildOptions(
                header: BuildHeaderOptions(checksum: ChecksumWriteOptions(seed: 1)),
                index: BuildIndexOptions(checksum: ChecksumWriteOptions(seed: 2)),
                dataChecksum: ChecksumWriteOptions(seed: 3)
            ))
        }

        test("BundleBuilder: removeEntry") {
            let builder = BundleBuilder(version: nil)
            _ = try builder.insertEntry(path: "/index.js", data: indexJs, contentType: nil, headers: nil)
            guard builder.removeEntry(path: "/index.js") else { throw Fail("removeEntry should return true") }
            guard !builder.containsEntry(path: "/index.js") else { throw Fail("entry should be removed") }
            guard !builder.removeEntry(path: "/index.js") else { throw Fail("second remove should return false") }
        }

        // ── Bundle ───────────────────────────────────────────────────────
        test("Bundle: version") {
            let bundle = try BundleBuilder(version: .v1).build(options: nil)
            guard bundle.descriptor().header().version() == .v1 else { throw Fail("version mismatch") }
        }

        test("Bundle: getData") {
            let builder = BundleBuilder(version: nil)
            _ = try builder.insertEntry(path: "/index.js", data: indexJs, contentType: nil, headers: nil)
            _ = try builder.insertEntry(path: "/index.html", data: indexHtml, contentType: nil, headers: nil)
            let bundle = try builder.build(options: nil)
            guard try bundle.getData(path: "/index.js") == indexJs else { throw Fail("index.js data mismatch") }
            guard try bundle.getData(path: "/index.html") == indexHtml else { throw Fail("index.html data mismatch") }
            guard try bundle.getData(path: "/not_exists") == nil else { throw Fail("/not_exists should return nil") }
        }

        test("Bundle: index entries") {
            let builder = BundleBuilder(version: nil)
            _ = try builder.insertEntry(path: "/index.js", data: indexJs, contentType: nil, headers: nil)
            _ = try builder.insertEntry(path: "/index.html", data: indexHtml, contentType: nil, headers: nil)
            let bundle = try builder.build(options: nil)
            let index = bundle.descriptor().index()
            guard index.entries().count == 2 else { throw Fail("expected 2 index entries") }
            guard index.containsPath(path: "/index.js") else { throw Fail("missing /index.js") }
            guard index.containsPath(path: "/index.html") else { throw Fail("missing /index.html") }
            guard !index.containsPath(path: "/not_exists") else { throw Fail("/not_exists should not exist") }
            guard index.getEntry(path: "/index.js") != nil else { throw Fail("getEntry returned nil") }
            guard index.getEntry(path: "/not_exists") == nil else { throw Fail("getEntry should return nil") }
        }

        test("Bundle: index entry metadata") {
            let builder = BundleBuilder(version: nil)
            _ = try builder.insertEntry(path: "/index.js", data: indexJs, contentType: "text/javascript", headers: ["x-custom": "value"])
            let bundle = try builder.build(options: nil)
            guard let entry = bundle.descriptor().index().getEntry(path: "/index.js") else { throw Fail("entry not found") }
            guard entry.contentType == "text/javascript" else { throw Fail("contentType=\(entry.contentType)") }
            guard entry.contentLength == UInt64(indexJs.count) else { throw Fail("contentLength mismatch") }
            guard entry.headers["x-custom"] == "value" else { throw Fail("header mismatch") }
        }

        test("Bundle: descriptor indexEntries") {
            let builder = BundleBuilder(version: nil)
            _ = try builder.insertEntry(path: "/index.js", data: indexJs, contentType: nil, headers: nil)
            let bundle = try builder.build(options: nil)
            let descriptor = bundle.descriptor()
            guard descriptor.indexEntries().count == 1 else { throw Fail("expected 1 entry") }
            guard descriptor.containsPath(path: "/index.js") else { throw Fail("missing /index.js") }
            guard !descriptor.containsPath(path: "/not_exists") else { throw Fail("/not_exists should not exist") }
            guard descriptor.getIndexEntry(path: "/index.js") != nil else { throw Fail("getIndexEntry returned nil") }
        }

        test("Bundle: getDataChecksum") {
            let builder = BundleBuilder(version: nil)
            _ = try builder.insertEntry(path: "/index.js", data: indexJs, contentType: nil, headers: nil)
            let bundle = try builder.build(options: nil)
            guard try bundle.getDataChecksum(path: "/index.js") != nil else { throw Fail("checksum should not be nil") }
            guard try bundle.getDataChecksum(path: "/not_exists") == nil else { throw Fail("checksum should be nil") }
        }

        // ── Read/Write ───────────────────────────────────────────────────
        test("Read/Write: bytes roundtrip") {
            let builder = BundleBuilder(version: nil)
            _ = try builder.insertEntry(path: "/index.js", data: indexJs, contentType: nil, headers: nil)
            _ = try builder.insertEntry(path: "/index.html", data: indexHtml, contentType: nil, headers: nil)
            let bundle = try builder.build(options: nil)
            let bytes = try writeBundleToBytes(bundle: bundle)
            guard !bytes.isEmpty else { throw Fail("bytes should not be empty") }
            let loaded = try readBundleFromBytes(data: bytes)
            guard loaded.descriptor().header().version() == .v1 else { throw Fail("version mismatch") }
            guard try loaded.getData(path: "/index.js") == indexJs else { throw Fail("index.js data mismatch") }
            guard try loaded.getData(path: "/index.html") == indexHtml else { throw Fail("index.html data mismatch") }
        }

        await testAsync("Read/Write: file roundtrip") {
            let tmpDir = FileManager.default.temporaryDirectory
                .appendingPathComponent("wvb-test-\(Int.random(in: 0..<Int.max))")
            try FileManager.default.createDirectory(at: tmpDir, withIntermediateDirectories: true)
            defer { try? FileManager.default.removeItem(at: tmpDir) }

            let builder = BundleBuilder(version: nil)
            _ = try builder.insertEntry(path: "/index.js", data: indexJs, contentType: nil, headers: nil)
            _ = try builder.insertEntry(path: "/index.html", data: indexHtml, contentType: nil, headers: nil)
            let bundle = try builder.build(options: nil)

            let filePath = tmpDir.appendingPathComponent("bundle.wvb").path
            _ = try await writeBundle(bundle: bundle, filepath: filePath)
            guard FileManager.default.fileExists(atPath: filePath) else { throw Fail("file not created") }

            let loaded = try await readBundle(filepath: filePath)
            guard loaded.descriptor().header().version() == .v1 else { throw Fail("version mismatch") }
            let index = loaded.descriptor().index()
            guard index.entries().count == 2 else { throw Fail("expected 2 entries") }
            guard index.containsPath(path: "/index.js") else { throw Fail("missing /index.js") }
            guard try loaded.getData(path: "/index.js") == indexJs else { throw Fail("index.js data mismatch") }
        }

        // ── Protocol ─────────────────────────────────────────────────────
        await testAsync("BundleProtocolHandler: 200 index.html") {
            try await withSource { source in
                let handler = BundleProtocolHandler(source: source)
                let response = try await handler.handle(method: .get, uri: "https://app.wvb/index.html", headers: nil)
                guard response.status == 200 else { throw Fail("status=\(response.status)") }
                guard !response.body.isEmpty else { throw Fail("body should not be empty") }
                guard response.headers["content-type"] != nil else { throw Fail("content-type header missing") }
            }
        }

        await testAsync("BundleProtocolHandler: 200 root redirect") {
            try await withSource { source in
                let handler = BundleProtocolHandler(source: source)
                let response = try await handler.handle(method: .get, uri: "https://app.wvb/", headers: nil)
                guard response.status == 200 else { throw Fail("status=\(response.status)") }
            }
        }

        await testAsync("BundleProtocolHandler: 404 not found") {
            try await withSource { source in
                let handler = BundleProtocolHandler(source: source)
                let response = try await handler.handle(method: .get, uri: "https://app.wvb/not_found.html", headers: nil)
                guard response.status == 404 else { throw Fail("status=\(response.status)") }
            }
        }

        await testAsync("BundleProtocolHandler: HEAD 200") {
            try await withSource { source in
                let handler = BundleProtocolHandler(source: source)
                let response = try await handler.handle(method: .head, uri: "https://app.wvb/index.html", headers: nil)
                guard response.status == 200 else { throw Fail("status=\(response.status)") }
                guard response.body.isEmpty else { throw Fail("HEAD response body must be empty") }
            }
        }

        // ── Builtin bundle ───────────────────────────────────────────────
        await testAsync("Source: fetch builtin bundle") {
            try await withBuiltinSource { source in
                let bundle = try await source.fetchBundle(bundleName: "app")
                guard try bundle.getData(path: "/index.html") != nil else {
                    throw Fail("index.html not found in builtin bundle")
                }
            }
        }

        await testAsync("Source: fetchDescriptor builtin") {
            try await withBuiltinSource { source in
                let descriptor = try await source.fetchDescriptor(bundleName: "app")
                guard descriptor.containsPath(path: "/index.html") else {
                    throw Fail("index.html not in descriptor")
                }
            }
        }

        await testAsync("BundleProtocolHandler: builtin 200 index.html") {
            try await withBuiltinSource { source in
                let handler = BundleProtocolHandler(source: source)
                let response = try await handler.handle(method: .get, uri: "https://app.wvb/index.html", headers: nil)
                guard response.status == 200 else { throw Fail("status=\(response.status)") }
                guard !response.body.isEmpty else { throw Fail("body should not be empty") }
            }
        }

        await testAsync("BundleProtocolHandler: exact path resolver does not rewrite to index.html") {
            try await withSource { source in
                let options = BundleProtocolOptions(pathResolver: .exact)
                let handler = BundleProtocolHandler(source: source, options: options)
                let served = try await handler.handle(method: .get, uri: "https://app.wvb/index.html", headers: nil)
                guard served.status == 200 else { throw Fail("status=\(served.status)") }
                let response = try await handler.handle(method: .get, uri: "https://app.wvb/", headers: nil)
                guard response.status == 404 else { throw Fail("status=\(response.status)") }
            }
        }

        await testAsync("BundleProtocolHandler: allowWvbSuffixOnly rejects other hosts") {
            try await withSource { source in
                let options = BundleProtocolOptions(
                    bundleResolver: .hostname(segment: .first, allowWvbSuffixOnly: true)
                )
                let handler = BundleProtocolHandler(source: source, options: options)
                let served = try await handler.handle(method: .get, uri: "https://app.wvb/index.html", headers: nil)
                guard served.status == 200 else { throw Fail("status=\(served.status)") }
                var didThrow = false
                do {
                    _ = try await handler.handle(method: .get, uri: "https://app.example.com/index.html", headers: nil)
                } catch {
                    didThrow = true
                }
                guard didThrow else { throw Fail("expected error for a host without the .wvb suffix") }
            }
        }

        test("ProxyProtocolHandler: init") {
            _ = ProxyProtocolHandler(hosts: ["myapp": "http://localhost:9999"])
        }

        await testAsync("BundleProtocolHandler: a request body is accepted") {
            try await withSource { source in
                let handler = BundleProtocolHandler(source: source)
                // The bundle protocol serves GET/HEAD only, but the body still travels over the FFI.
                let response = try await handler.handle(
                    method: .post,
                    uri: "https://app.wvb/index.html",
                    headers: nil,
                    body: Data(#"{"hello":"world"}"#.utf8)
                )
                guard response.status == 405 else { throw Fail("status=\(response.status)") }
            }
        }

        await testAsync("ProxyProtocolHandler: unknown host error") {
            let handler = ProxyProtocolHandler(hosts: ["known": "http://localhost:9999"])
            var didThrow = false
            do {
                _ = try await handler.handle(method: .get, uri: "https://unknown.wvb/index.html", headers: nil)
            } catch {
                didThrow = true
            }
            guard didThrow else { throw Fail("expected error for unknown host") }
        }

        await testAsync("ProxyProtocolHandler: custom resolver receives the uri") {
            let resolver = RecordingProxyResolver()
            let handler = ProxyProtocolHandler.custom(resolver: resolver)
            var didThrow = false
            do {
                _ = try await handler.handle(method: .get, uri: "https://app.wvb/index.html", headers: nil)
            } catch {
                didThrow = true
            }
            guard didThrow else { throw Fail("expected error when the resolver returns nil") }
            guard resolver.seen == "https://app.wvb/index.html" else {
                throw Fail("resolver saw uri=\(resolver.seen ?? "none")")
            }
        }

        results = newResults
        isRunning = false
    }

    private func withBuiltinSource(_ block: (Source) async throws -> Void) async throws {
        print("resource url: \(Bundle.main.resourceURL?.absoluteString ?? "none")")
        guard let builtinDir = Bundle.main.resourceURL?.appendingPathComponent("assets").appendingPathComponent("bundles").appendingPathComponent("builtin") else {
            throw Fail("builtin resource directory not found in app bundle")
        }
        let remoteDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("wvb-builtin-remote-\(Int.random(in: 0..<Int.max))")
        try FileManager.default.createDirectory(at: remoteDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: remoteDir) }

        let emptyManifest = #"{"manifestVersion":1,"bundles":{}}"#
        try Data(emptyManifest.utf8).write(to: remoteDir.appendingPathComponent("manifest.json"))

        let source = try Source(config: SourceConfig(
            builtinDir: builtinDir.path,
            remoteDir: remoteDir.path,
            builtinManifestFilepath: nil,
            remoteManifestFilepath: nil
        ))
        try await block(source)
    }

    private func withSource(_ block: (Source) async throws -> Void) async throws {
        let tmpDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("wvb-fixture-\(Int.random(in: 0..<Int.max))")
        let remoteDir = tmpDir.appendingPathComponent("remote")
        let appDir = remoteDir.appendingPathComponent("app")
        try FileManager.default.createDirectory(at: appDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tmpDir) }

        let manifest = #"{"manifestVersion":1,"bundles":{"app":{"versions":{"1.0.0":{}},"currentVersion":"1.0.0"}}}"#
        try Data(manifest.utf8).write(to: remoteDir.appendingPathComponent("manifest.json"))

        let builder = BundleBuilder(version: nil)
        _ = try builder.insertEntry(path: "/index.html", data: indexHtml, contentType: "text/html", headers: nil)
        let bundle = try builder.build(options: nil)
        let bundleBytes = try writeBundleToBytes(bundle: bundle)
        try bundleBytes.write(to: appDir.appendingPathComponent("1.0.0.wvb"))

        let builtinDir = tmpDir.appendingPathComponent("builtin")
        try FileManager.default.createDirectory(at: builtinDir, withIntermediateDirectories: true)

        let source = try Source(config: SourceConfig(
            builtinDir: builtinDir.path,
            remoteDir: remoteDir.path,
            builtinManifestFilepath: nil,
            remoteManifestFilepath: nil
        ))
        try await block(source)
    }
}

private struct Fail: LocalizedError {
    let errorDescription: String?
    init(_ message: String) { errorDescription = message }
}

/// Records the uri it is asked to resolve, and never proxies.
private final class RecordingProxyResolver: ProxyResolver, @unchecked Sendable {
    private let lock = NSLock()
    private var uri: String?

    var seen: String? {
        lock.lock()
        defer { lock.unlock() }
        return uri
    }

    func resolve(uri: String) async -> String? {
        lock.lock()
        self.uri = uri
        lock.unlock()
        return nil
    }
}
