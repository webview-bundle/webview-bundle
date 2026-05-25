import Foundation

#if canImport(WebKit)
import WebKit
import WebViewBundleLibrary

/// A `WKURLSchemeHandler` that serves WebViewBundle resources for a single
/// scheme by routing requests to a UniFFI handler.
///
/// WebKit invokes the handler methods on the main thread. The (suspending) FFI
/// handler runs off the main thread; its result is fed back to the task on the
/// main thread, and skipped if the task was already stopped.
final class WebViewBundleSchemeHandler: NSObject, WKURLSchemeHandler {
    private let handler: WebViewBundleRequestHandler

    // Touched only on the main thread (WebKit calls start/stop there, and the
    // completion below hops back to main).
    private var activeTasks = Set<ObjectIdentifier>()

    init(handler: WebViewBundleRequestHandler) {
        self.handler = handler
    }

    func webView(_ webView: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        let id = ObjectIdentifier(urlSchemeTask)
        activeTasks.insert(id)

        let request = urlSchemeTask.request
        let method = HttpMethod.from(request.httpMethod)
        let uri = request.url?.absoluteString ?? ""
        let headers = request.allHTTPHeaderFields
        let url = request.url ?? URL(string: "about:blank")!

        Task {
            do {
                let response = try await self.handler.handle(method: method, uri: uri, headers: headers)
                self.complete(urlSchemeTask, id: id, url: url, result: .success(response))
            } catch {
                self.complete(urlSchemeTask, id: id, url: url, result: .failure(error))
            }
        }
    }

    func webView(_ webView: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {
        activeTasks.remove(ObjectIdentifier(urlSchemeTask))
    }

    private func complete(
        _ task: WKURLSchemeTask,
        id: ObjectIdentifier,
        url: URL,
        result: Result<HttpResponse, Error>
    ) {
        DispatchQueue.main.async {
            // Stop and completion are both serialized on the main queue, so this
            // check is race-free: a stopped task is never fed.
            guard self.activeTasks.remove(id) != nil else { return }
            switch result {
            case let .success(response):
                task.didReceive(response.makeURLResponse(url: url))
                task.didReceive(response.body)
                task.didFinish()
            case let .failure(error):
                task.didFailWithError(error)
            }
        }
    }
}
#endif
