import Foundation

/// Binds a URL scheme handled inside a `WKWebView` to a WebViewBundle request
/// handler.
///
/// This mirrors the protocol model used by the `@wvb/electron` and `wvb-tauri`
/// packages: each protocol owns a scheme, and requests to that scheme are routed
/// to the matching handler which resolves them against the bundle source or a
/// local server.
///
/// The bundle name is resolved from the first label of the request host, e.g.
/// `app://app.wvb/index.html` -> bundle `"app"`, path `"/index.html"`.
///
/// `WKWebView` only allows scheme handlers for non-reserved schemes, so use a
/// custom scheme (not `http`/`https`).
public enum WebViewBundleProtocol {
    /// Serves entries from the WebViewBundle source, backed by a
    /// `BundleUrlHandler`.
    case bundle(scheme: String)

    /// Proxies requests to local HTTP servers, backed by a `LocalUrlHandler`.
    /// `hosts` maps a virtual host to a local base URL, e.g.
    /// `["myapp": "http://localhost:8080"]`.
    case local(scheme: String, hosts: [String: String])

    /// The URL scheme this protocol handles.
    public var scheme: String {
        switch self {
        case let .bundle(scheme): return scheme
        case let .local(scheme, _): return scheme
        }
    }
}
