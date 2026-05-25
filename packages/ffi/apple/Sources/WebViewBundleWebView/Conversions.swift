import Foundation
import WebViewBundleLibrary

extension HttpMethod {
    /// Maps a `URLRequest.httpMethod` string to the FFI `HttpMethod`.
    /// Defaults to `.get` for unknown or missing methods.
    static func from(_ method: String?) -> HttpMethod {
        switch method?.uppercased() {
        case "GET": return .get
        case "HEAD": return .head
        case "OPTIONS": return .options
        case "POST": return .post
        case "PUT": return .put
        case "PATCH": return .patch
        case "DELETE": return .delete
        case "TRACE": return .trace
        case "CONNECT": return .connect
        default: return .get
        }
    }
}

extension HttpResponse {
    /// Builds an `HTTPURLResponse` for [url] from this response's status and
    /// headers.
    func makeURLResponse(url: URL) -> HTTPURLResponse {
        HTTPURLResponse(
            url: url,
            statusCode: Int(status),
            httpVersion: "HTTP/1.1",
            headerFields: headers
        ) ?? HTTPURLResponse(
            url: url,
            statusCode: Int(status),
            httpVersion: "HTTP/1.1",
            headerFields: nil
        )!
    }
}
