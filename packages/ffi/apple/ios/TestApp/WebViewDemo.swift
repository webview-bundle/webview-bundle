import SwiftUI
import WebKit
import WebViewBundleLibrary
import WebViewBundleWebView

/// Minimal manual check for the WebView integration: builds a bundle in memory,
/// serves it through `WebViewBundle`, and loads it in a `WKWebView`.
///
/// A successful screen shows a green "WebViewBundle" heading (CSS applied) and a
/// status line printed by JS that includes `app://app.wvb` as the origin —
/// proving HTML, CSS and JS sub-resources are all served from the bundle.
@MainActor
final class WebViewDemoModel: ObservableObject {
    @Published private(set) var webView: WKWebView?
    @Published private(set) var error: String?

    // Retain the bundle for the web view's lifetime; it owns the scheme handlers.
    private var wvb: WebViewBundle?

    func setup() {
        guard webView == nil, error == nil else { return }
        do {
            let source = try makeSource()
            let wvb = WebViewBundle(source: source, protocols: [.bundle(scheme: "app")])
            let webView = wvb.makeWebView()
            webView.load(URLRequest(url: URL(string: "app://app.wvb/index.html")!))
            self.wvb = wvb
            self.webView = webView
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func makeSource() throws -> BundleSource {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent("wvb-webview-demo-\(UUID().uuidString)")
        let remote = tmp.appendingPathComponent("remote")
        let appDir = remote.appendingPathComponent("app")
        let builtin = tmp.appendingPathComponent("builtin")
        try FileManager.default.createDirectory(at: appDir, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: builtin, withIntermediateDirectories: true)

        let manifest = #"{"manifestVersion":1,"entries":{"app":{"versions":{"1.0.0":{}},"currentVersion":"1.0.0"}}}"#
        try Data(manifest.utf8).write(to: remote.appendingPathComponent("manifest.json"))

        let builder = BundleBuilder(version: nil)
        _ = try builder.insertEntry(path: "/index.html", data: Data(Self.indexHTML.utf8), contentType: "text/html", headers: nil)
        _ = try builder.insertEntry(path: "/style.css", data: Data(Self.styleCSS.utf8), contentType: "text/css", headers: nil)
        _ = try builder.insertEntry(path: "/app.js", data: Data(Self.appJS.utf8), contentType: "text/javascript", headers: nil)
        let bundle = try builder.build(options: nil)
        try writeBundleToBytes(bundle: bundle).write(to: appDir.appendingPathComponent("app_1.0.0.wvb"))

        return BundleSource(config: BundleSourceConfig(
            builtinDir: builtin.path,
            remoteDir: remote.path,
            builtinManifestFilepath: nil,
            remoteManifestFilepath: nil
        ))
    }

    private static let indexHTML = """
    <!DOCTYPE html>
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

    private static let styleCSS = """
    body { font-family: -apple-system, sans-serif; padding: 24px; }
    #title { color: #2E7D32; }
    #status.ok { color: #2E7D32; font-weight: bold; }
    """

    private static let appJS = """
    var el = document.getElementById('status');
    el.textContent = 'OK · served from ' + location.origin;
    el.className = 'ok';
    """
}

struct WebViewContainer: UIViewRepresentable {
    let webView: WKWebView
    func makeUIView(context: Context) -> WKWebView { webView }
    func updateUIView(_ uiView: WKWebView, context: Context) {}
}

struct WebViewDemoView: View {
    @StateObject private var model = WebViewDemoModel()

    var body: some View {
        Group {
            if let webView = model.webView {
                WebViewContainer(webView: webView)
                    .ignoresSafeArea(edges: .bottom)
            } else if let error = model.error {
                Text(error)
                    .foregroundColor(.red)
                    .padding()
            } else {
                ProgressView()
            }
        }
        .onAppear { model.setup() }
    }
}
