import Foundation
import WebViewBundleLibrary

#if canImport(WebKit)
import WebKit
#endif

/// Serves WebViewBundle resources to a system `WKWebView`.
///
/// `WebViewBundle` is the iOS/macOS counterpart to the `@wvb/electron` and
/// `wvb-tauri` packages. It wires one or more ``WebViewBundleProtocol``s to a
/// `WKWebViewConfiguration` via `WKURLSchemeHandler`: requests whose scheme
/// matches a registered protocol are resolved from the bundle ``source`` (or
/// proxied to a local server) instead of hitting the network.
///
/// ```swift
/// let wvb = WebViewBundle(
///     source: BundleSource(config: BundleSourceConfig(
///         builtinDir: builtinDir.path,
///         remoteDir: remoteDir.path,
///         builtinManifestFilepath: nil,
///         remoteManifestFilepath: nil
///     )),
///     protocols: [.bundle(scheme: "app")]
/// )
/// let webView = wvb.makeWebView()
/// webView.load(URLRequest(url: URL(string: "app://app.wvb/index.html")!))
/// ```
///
/// Keep a strong reference to the `WebViewBundle` for the lifetime of the web
/// view; it owns the scheme handlers.
public final class WebViewBundle {
    /// The bundle source requests are served from.
    public let source: BundleSource
    /// Optional remote client, exposed for update flows.
    public let remote: Remote?
    /// Optional updater, exposed for update flows.
    public let updater: Updater?

    #if canImport(WebKit)
    private let schemeHandlers: [(scheme: String, handler: WebViewBundleSchemeHandler)]
    #endif

    /// - Parameters:
    ///   - source: the bundle source requests are served from.
    ///   - protocols: the protocols to register; each must use a unique,
    ///     non-reserved scheme.
    ///   - remote: optional remote client.
    ///   - updater: optional updater.
    public init(
        source: BundleSource,
        protocols: [WebViewBundleProtocol],
        remote: Remote? = nil,
        updater: Updater? = nil
    ) {
        self.source = source
        self.remote = remote
        self.updater = updater

        #if canImport(WebKit)
        var seen = Set<String>()
        self.schemeHandlers = protocols.map { proto in
            let scheme = proto.scheme
            precondition(!scheme.isEmpty, "protocol scheme must not be empty")
            precondition(seen.insert(scheme).inserted, "duplicate protocol scheme: \(scheme)")
            let handler: WebViewBundleRequestHandler
            switch proto {
            case .bundle:
                handler = BundleUrlHandler(source: source)
            case let .local(_, hosts):
                handler = LocalUrlHandler(hosts: hosts)
            }
            return (scheme: scheme, handler: WebViewBundleSchemeHandler(handler: handler))
        }
        #endif
    }

    #if canImport(WebKit)
    /// The schemes this instance intercepts.
    public var schemes: [String] { schemeHandlers.map { $0.scheme } }

    /// Registers the bundle scheme handlers on [configuration].
    public func install(on configuration: WKWebViewConfiguration) {
        for (scheme, handler) in schemeHandlers {
            configuration.setURLSchemeHandler(handler, forURLScheme: scheme)
        }
    }

    /// A fresh `WKWebViewConfiguration` with the bundle scheme handlers
    /// installed.
    public func makeConfiguration() -> WKWebViewConfiguration {
        let configuration = WKWebViewConfiguration()
        install(on: configuration)
        return configuration
    }

    /// A fresh `WKWebView` configured to serve the registered bundles.
    public func makeWebView(frame: CGRect = .zero) -> WKWebView {
        WKWebView(frame: frame, configuration: makeConfiguration())
    }
    #endif
}
