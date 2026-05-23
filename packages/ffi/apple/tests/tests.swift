import Testing
import Foundation
import Darwin
@testable import WebViewBundleLibrary

private let indexHtmlData = Data("<!DOCTYPE html>".utf8)
private let indexJsData = Data("console.log('hello')".utf8)

// ── HttpResponse ──────────────────────────────────────────────────────────────

@Test func init_http_response() {
    let response = HttpResponse(status: 200, headers: ["Content-Type": "text/html"], body: Data())
    #expect(response.status == 200)
    #expect(response.headers["Content-Type"] == "text/html")
    #expect(response.body.isEmpty)
}

// ── BundleBuilder ─────────────────────────────────────────────────────────────

@Test func build_bundle() throws {
    let builder = BundleBuilder(version: nil)
    #expect(builder.version() == .v1)
    #expect(builder.entryPaths().isEmpty)

    let ins1 = try builder.insertEntry(path: "/index.js", data: indexJsData, contentType: nil, headers: nil)
    #expect(ins1 == false)
    let ins2 = try builder.insertEntry(path: "/index.html", data: indexHtmlData, contentType: nil, headers: nil)
    #expect(ins2 == false)
    let ins3 = try builder.insertEntry(path: "/index.html", data: indexHtmlData, contentType: nil, headers: nil)
    #expect(ins3 == true) // same path → replaced

    #expect(builder.entryPaths().count == 2)
    #expect(builder.containsEntry(path: "/index.js"))
    #expect(builder.containsEntry(path: "/index.html"))
    #expect(!builder.containsEntry(path: "/not_exists"))

    _ = try builder.build(options: nil)
}

@Test func build_bundle_with_version() throws {
    let builder = BundleBuilder(version: .v1)
    #expect(builder.version() == .v1)
    _ = try builder.build(options: nil)
}

@Test func build_bundle_with_options() throws {
    let builder = BundleBuilder(version: nil)
    _ = try builder.insertEntry(path: "/index.js", data: indexJsData, contentType: nil, headers: nil)
    _ = try builder.build(options: BuildOptions(
        header: BuildHeaderOptions(checksumSeed: 1),
        index: BuildIndexOptions(checksumSeed: 2),
        dataChecksumSeed: 3
    ))
}

@Test func remove_entry() throws {
    let builder = BundleBuilder(version: nil)
    _ = try builder.insertEntry(path: "/index.js", data: indexJsData, contentType: nil, headers: nil)
    #expect(builder.containsEntry(path: "/index.js"))
    #expect(builder.removeEntry(path: "/index.js"))
    #expect(!builder.containsEntry(path: "/index.js"))
    #expect(!builder.removeEntry(path: "/index.js")) // already gone
}

// ── Bundle ────────────────────────────────────────────────────────────────────

@Test func bundle_version() throws {
    let builder = BundleBuilder(version: .v1)
    let bundle = try builder.build(options: nil)
    #expect(bundle.descriptor().header().version() == .v1)
}

@Test func bundle_get_data() throws {
    let builder = BundleBuilder(version: nil)
    _ = try builder.insertEntry(path: "/index.js", data: indexJsData, contentType: nil, headers: nil)
    _ = try builder.insertEntry(path: "/index.html", data: indexHtmlData, contentType: nil, headers: nil)
    let bundle = try builder.build(options: nil)

    #expect(try bundle.getData(path: "/index.js") == indexJsData)
    #expect(try bundle.getData(path: "/index.html") == indexHtmlData)
    #expect(try bundle.getData(path: "/not_exists") == nil)
}

@Test func bundle_index_entries() throws {
    let builder = BundleBuilder(version: nil)
    _ = try builder.insertEntry(path: "/index.js", data: indexJsData, contentType: nil, headers: nil)
    _ = try builder.insertEntry(path: "/index.html", data: indexHtmlData, contentType: nil, headers: nil)
    let bundle = try builder.build(options: nil)
    let index = bundle.descriptor().index()

    #expect(index.entries().count == 2)
    #expect(index.containsPath(path: "/index.js"))
    #expect(index.containsPath(path: "/index.html"))
    #expect(!index.containsPath(path: "/not_exists"))
    #expect(index.getEntry(path: "/index.js") != nil)
    #expect(index.getEntry(path: "/not_exists") == nil)
}

@Test func bundle_index_entry_metadata() throws {
    let builder = BundleBuilder(version: nil)
    _ = try builder.insertEntry(path: "/index.js", data: indexJsData, contentType: "text/javascript", headers: ["x-custom": "value"])
    let bundle = try builder.build(options: nil)
    let entry = bundle.descriptor().index().getEntry(path: "/index.js")

    #expect(entry != nil)
    #expect(entry?.contentType == "text/javascript")
    #expect(entry?.contentLength == UInt64(indexJsData.count))
    #expect(entry?.headers["x-custom"] == "value")
}

@Test func bundle_descriptor_index_entries() throws {
    let builder = BundleBuilder(version: nil)
    _ = try builder.insertEntry(path: "/index.js", data: indexJsData, contentType: nil, headers: nil)
    let bundle = try builder.build(options: nil)
    let descriptor = bundle.descriptor()

    #expect(descriptor.indexEntries().count == 1)
    #expect(descriptor.containsPath(path: "/index.js"))
    #expect(!descriptor.containsPath(path: "/not_exists"))
    #expect(descriptor.getIndexEntry(path: "/index.js") != nil)
    #expect(descriptor.getIndexEntry(path: "/not_exists") == nil)
}

@Test func bundle_get_data_checksum() throws {
    let builder = BundleBuilder(version: nil)
    _ = try builder.insertEntry(path: "/index.js", data: indexJsData, contentType: nil, headers: nil)
    let bundle = try builder.build(options: nil)

    #expect(try bundle.getDataChecksum(path: "/index.js") != nil)
    #expect(try bundle.getDataChecksum(path: "/not_exists") == nil)
}

// ── Read/Write ────────────────────────────────────────────────────────────────

@Test func read_write_bundle_bytes() throws {
    let builder = BundleBuilder(version: nil)
    _ = try builder.insertEntry(path: "/index.js", data: indexJsData, contentType: nil, headers: nil)
    _ = try builder.insertEntry(path: "/index.html", data: indexHtmlData, contentType: nil, headers: nil)
    let bundle = try builder.build(options: nil)

    let bytes = try writeBundleToBytes(bundle: bundle)
    #expect(!bytes.isEmpty)

    let loaded = try readBundleFromBytes(data: bytes)
    #expect(loaded.descriptor().header().version() == .v1)
    #expect(try loaded.getData(path: "/index.js") == indexJsData)
    #expect(try loaded.getData(path: "/index.html") == indexHtmlData)
}

@Test func read_write_bundle_file() async throws {
    let tmpDir = FileManager.default.temporaryDirectory
        .appendingPathComponent("wvb-test-\(Int.random(in: 0..<Int.max))", isDirectory: true)
    try FileManager.default.createDirectory(at: tmpDir, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: tmpDir) }

    let builder = BundleBuilder(version: nil)
    _ = try builder.insertEntry(path: "/index.js", data: indexJsData, contentType: nil, headers: nil)
    _ = try builder.insertEntry(path: "/index.html", data: indexHtmlData, contentType: nil, headers: nil)
    let bundle = try builder.build(options: nil)

    let filePath = tmpDir.appendingPathComponent("bundle.wvb").path
    _ = try await writeBundle(bundle: bundle, filepath: filePath)

    let loaded = try await readBundle(filepath: filePath)
    #expect(loaded.descriptor().header().version() == .v1)
    let index = loaded.descriptor().index()
    #expect(index.entries().count == 2)
    #expect(index.containsPath(path: "/index.js"))
    #expect(index.containsPath(path: "/index.html"))
    #expect(try loaded.getData(path: "/index.js") == indexJsData)
}

// ── Remote ────────────────────────────────────────────────────────────────────

@Test func remote_list_bundles() async throws {
    try await withMockServer { port in
        let remote = try Remote(endpoint: "http://localhost:\(port)")
        let bundles = try await remote.listBundles(channel: nil)
        #expect(bundles.count == 1)
        #expect(bundles[0].name == "bundle1")
        #expect(bundles[0].version == "1.0.0")
    }
}

@Test func remote_get_info() async throws {
    try await withMockServer { port in
        let remote = try Remote(endpoint: "http://localhost:\(port)")
        let info = try await remote.getInfo(bundleName: "bundle1", channel: nil)
        #expect(info.name == "bundle1")
        #expect(info.version == "1.0.0")
    }
}

@Test func remote_download() async throws {
    try await withMockServer { port in
        let remote = try Remote(endpoint: "http://localhost:\(port)")
        let result = try await remote.download(bundleName: "bundle1", channel: nil)
        #expect(result.info.name == "bundle1")
        #expect(result.info.version == "1.0.0")
        #expect(try result.bundle.getData(path: "/index.html") == indexHtmlData)
    }
}

@Test func remote_download_version() async throws {
    try await withMockServer { port in
        let remote = try Remote(endpoint: "http://localhost:\(port)")
        let result = try await remote.downloadVersion(bundleName: "bundle1", version: "1.0.0")
        #expect(result.info.name == "bundle1")
        #expect(result.info.version == "1.0.0")
        #expect(try result.bundle.getData(path: "/index.html") == indexHtmlData)
    }
}

@Test func remote_download_version_forbidden() async throws {
    try await withMockServer(allowOnlyLatest: true) { port in
        let remote = try Remote(endpoint: "http://localhost:\(port)")
        var didThrow = false
        do {
            _ = try await remote.downloadVersion(bundleName: "bundle1", version: "1.0.0")
        } catch {
            didThrow = true
        }
        #expect(didThrow)
    }
}

@Test func remote_bundle_not_found() async throws {
    try await withMockServer { port in
        let remote = try Remote(endpoint: "http://localhost:\(port)")
        var didThrow = false
        do {
            _ = try await remote.download(bundleName: "not_found", channel: nil)
        } catch {
            didThrow = true
        }
        #expect(didThrow)
    }
}

// ── Mock server helpers ───────────────────────────────────────────────────────

private func makeBundleBytes() throws -> Data {
    let builder = BundleBuilder(version: nil)
    _ = try builder.insertEntry(path: "/index.html", data: indexHtmlData, contentType: "text/html", headers: nil)
    return try writeBundleToBytes(bundle: builder.build(options: nil))
}

private func withMockServer(
    allowOnlyLatest: Bool = false,
    _ block: (Int) async throws -> Void
) async throws {
    let bundleBytes = try makeBundleBytes()
    let server = try TestHttpServer(bundleBytes: bundleBytes, allowOnlyLatest: allowOnlyLatest)
    server.start()
    defer { server.stop() }
    try await block(server.port)
}

// ── TestHttpServer ────────────────────────────────────────────────────────────

final class TestHttpServer: @unchecked Sendable {
    let port: Int
    private let socketFd: Int32
    private let bundleBytes: Data
    private let allowOnlyLatest: Bool

    init(bundleBytes: Data, allowOnlyLatest: Bool = false) throws {
        self.bundleBytes = bundleBytes
        self.allowOnlyLatest = allowOnlyLatest

        let fd = Darwin.socket(AF_INET, SOCK_STREAM, 0)
        guard fd >= 0 else {
            throw NSError(domain: "TestHttpServer", code: 1, userInfo: [NSLocalizedDescriptionKey: "socket() failed"])
        }
        socketFd = fd

        var opt: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &opt, socklen_t(MemoryLayout<Int32>.size))

        var addr = sockaddr_in()
        addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = 0 // OS assigns port
        addr.sin_addr.s_addr = INADDR_ANY

        let bindResult = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        guard bindResult == 0 else {
            Darwin.close(fd)
            throw NSError(domain: "TestHttpServer", code: 2, userInfo: [NSLocalizedDescriptionKey: "bind() failed: \(errno)"])
        }

        Darwin.listen(fd, 10)

        var boundAddr = sockaddr_in()
        var addrLen = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &boundAddr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                getsockname(fd, $0, &addrLen)
            }
        }
        port = Int(UInt16(bigEndian: boundAddr.sin_port))
    }

    func start() {
        Thread.detachNewThread { [weak self] in
            guard let self else { return }
            while true {
                let clientFd = Darwin.accept(self.socketFd, nil, nil)
                guard clientFd >= 0 else { break }
                let bytes = self.bundleBytes
                let forbidden = self.allowOnlyLatest
                Thread.detachNewThread {
                    self.handleClient(clientFd, bundleBytes: bytes, allowOnlyLatest: forbidden)
                }
            }
        }
    }

    func stop() {
        Darwin.close(socketFd)
    }

    private func handleClient(_ clientFd: Int32, bundleBytes: Data, allowOnlyLatest: Bool) {
        defer { Darwin.close(clientFd) }

        var buf = [UInt8](repeating: 0, count: 8192)
        let n = Darwin.read(clientFd, &buf, 8192)
        guard n > 0 else { return }

        let req = String(bytes: buf.prefix(n), encoding: .utf8) ?? ""
        let lines = req.components(separatedBy: "\r\n")
        guard let requestLine = lines.first else { return }
        let parts = requestLine.split(separator: " ", maxSplits: 2)
        guard parts.count >= 2 else { return }

        let method = String(parts[0])
        let rawPath = String(parts[1])
        let path = rawPath.components(separatedBy: "?").first ?? rawPath

        let segments = path
            .trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            .components(separatedBy: "/")
            .filter { !$0.isEmpty }

        guard segments.first == "bundles" else {
            sendResponse(clientFd, status: 404, extraHeaders: [], body: Data())
            return
        }

        switch segments.count {
        case 1:
            // GET /bundles → list
            let body = Data(#"[{"name":"bundle1","version":"1.0.0"}]"#.utf8)
            sendResponse(clientFd, status: 200, extraHeaders: [("Content-Type", "application/json")], body: body)

        case 2:
            // HEAD or GET /bundles/{name}
            let bundleName = segments[1]
            guard bundleName == "bundle1" else {
                sendResponse(clientFd, status: 404, extraHeaders: [], body: Data())
                return
            }
            let hdrs: [(String, String)] = [
                ("Content-Type", "application/webview-bundle"),
                ("webview-bundle-name", bundleName),
                ("webview-bundle-version", "1.0.0")
            ]
            if method == "HEAD" {
                sendResponse(clientFd, status: 200, extraHeaders: hdrs, body: Data(), headOnly: true, headBodySize: bundleBytes.count)
            } else {
                sendResponse(clientFd, status: 200, extraHeaders: hdrs, body: bundleBytes)
            }

        case 3:
            // GET /bundles/{name}/{version}
            let bundleName = segments[1]
            let version = segments[2]
            if allowOnlyLatest {
                sendResponse(clientFd, status: 403, extraHeaders: [], body: Data())
            } else if bundleName == "bundle1" && version == "1.0.0" {
                let hdrs: [(String, String)] = [
                    ("Content-Type", "application/webview-bundle"),
                    ("webview-bundle-name", bundleName),
                    ("webview-bundle-version", version)
                ]
                sendResponse(clientFd, status: 200, extraHeaders: hdrs, body: bundleBytes)
            } else {
                sendResponse(clientFd, status: 404, extraHeaders: [], body: Data())
            }

        default:
            sendResponse(clientFd, status: 404, extraHeaders: [], body: Data())
        }
    }

    private func sendResponse(
        _ fd: Int32,
        status: Int,
        extraHeaders: [(String, String)],
        body: Data,
        headOnly: Bool = false,
        headBodySize: Int? = nil
    ) {
        let statusText: String
        switch status {
        case 200: statusText = "OK"
        case 403: statusText = "Forbidden"
        default: statusText = "Not Found"
        }

        var resp = "HTTP/1.1 \(status) \(statusText)\r\n"
        for (k, v) in extraHeaders {
            resp += "\(k): \(v)\r\n"
        }
        let contentLength = headOnly ? (headBodySize ?? 0) : body.count
        resp += "Content-Length: \(contentLength)\r\n"
        resp += "Connection: close\r\n\r\n"

        let headerData = Data(resp.utf8)
        headerData.withUnsafeBytes { ptr in
            _ = Darwin.write(fd, ptr.baseAddress!, ptr.count)
        }
        if !headOnly && !body.isEmpty {
            body.withUnsafeBytes { ptr in
                _ = Darwin.write(fd, ptr.baseAddress!, ptr.count)
            }
        }
    }
}
