// The hardened web view the shared editor bundle runs in, for the composer and the signature
// editor alike (docs/composer-security.md, the Apple column): JavaScript for this document only,
// no windows, a non-persistent store, every remote load blocked, and no navigation after the first.
//
// It also decides when that web view exists. A SwiftUI view's initialiser runs on every render of
// its parent, and `@State` keeps only the first value it is given, so whatever an editor builds in
// its own initialiser is built again and thrown away on every render. A loaded web view is a
// WebContent process. So an editor builds nothing until it is put on screen: `mount()` belongs to
// the representable, creates the web view once, and every later call answers that one.

import Foundation
import WebKit

enum EditorHostError: Error {
    /// The editor has not been put on screen, so there is no document to ask.
    case notMounted
}

@MainActor
final class EditorHost: NSObject, WKNavigationDelegate {
    /// The editor's request channel (docs/composer-security.md, Gate 2).
    let channel = ComposerHostChannel()
    /// Runs each time the bundle finishes loading, which with navigation blocked is once.
    var onLoad: () -> Void = {}

    /// `nil` until `mount()`, and the same view after it.
    private(set) var webView: WKWebView?
    private let makeView: (WKWebViewConfiguration) -> WKWebView
    private let fallbackHTML: String
    private var expectingInitialLoad = true

    /// - Parameters:
    ///   - makeView: builds the web view from the hardened configuration, for a host that needs a
    ///     subclass of its own (the composer's `EditorWebView`).
    ///   - fallbackHTML: the document loaded when the bundle cannot be found, which defines the
    ///     functions its host reads back so a read answers empty rather than failing.
    init(
        fallbackHTML: String,
        makeView: @escaping (WKWebViewConfiguration) -> WKWebView = {
            WKWebView(frame: .zero, configuration: $0)
        }
    ) {
        self.fallbackHTML = fallbackHTML
        self.makeView = makeView
    }

    /// The web view, built and loading the first time it is asked for. `configure` runs only then,
    /// before anything has loaded.
    func mount(configure: (WKWebView) -> Void = { _ in }) -> WKWebView {
        if let webView { return webView }
        let configuration = WKWebViewConfiguration()
        let preferences = WKWebpagePreferences()
        preferences.allowsContentJavaScript = true
        configuration.defaultWebpagePreferences = preferences
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        configuration.websiteDataStore = .nonPersistent()
        let view = makeView(configuration)
        view.navigationDelegate = self
        view.allowsBackForwardNavigationGestures = false
        channel.install(on: view)
        configure(view)
        webView = view
        // The bundle loads behind the native barrier (`ComposerRemoteBlock`). If the rule list
        // cannot be compiled the bundle's CSP still blocks remote loads, so the editor loads anyway
        // rather than being left blank.
        ComposerRemoteBlock.ruleList { [weak self] ruleList in
            guard let self, let view = self.webView else { return }
            if let ruleList {
                view.configuration.userContentController.add(ruleList)
            }
            let asset = EditorAsset.load(fallbackHTML: self.fallbackHTML)
            view.loadHTMLString(asset.html, baseURL: asset.baseURL)
        }
        return view
    }

    /// Runs `script` in the document. Before `mount()` there is no document, and the completion is
    /// told so.
    func evaluate(
        _ script: String,
        completion: (@MainActor (Any?, (any Error)?) -> Void)? = nil
    ) {
        guard let webView else {
            completion?(nil, EditorHostError.notMounted)
            return
        }
        webView.evaluateJavaScript(script, completionHandler: completion)
    }

    /// Encodes `value` as a JavaScript string literal (quoted and escaped), so it can be passed into
    /// a script as an argument without breaking out of it.
    static func jsString(_ value: String) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: value, options: .fragmentsAllowed),
              let literal = String(data: data, encoding: .utf8)
        else {
            return "\"\""
        }
        return literal
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        onLoad()
    }

    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void
    ) {
        if expectingInitialLoad {
            expectingInitialLoad = false
            decisionHandler(.allow)
        } else {
            decisionHandler(.cancel)
        }
    }
}

/// The shared editor bundle, found by three routes: the SPM resource, the app bundle for a host that
/// copies it there, and the source tree for a `swift run` from the checkout.
private struct EditorAsset {
    let html: String
    let baseURL: URL?

    static func load(fallbackHTML: String) -> EditorAsset {
        for bundle in [Bundle.module, Bundle.main] {
            if let bundleURL = bundle.url(
                forResource: "editor",
                withExtension: "html",
                subdirectory: "composer"
            ), let html = try? String(contentsOf: bundleURL, encoding: .utf8) {
                return EditorAsset(html: html, baseURL: bundleURL.deletingLastPathComponent())
            }
        }
        let sourceURL = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("composer/editor.html")
        if let html = try? String(contentsOf: sourceURL, encoding: .utf8) {
            return EditorAsset(html: html, baseURL: sourceURL.deletingLastPathComponent())
        }
        return EditorAsset(html: fallbackHTML, baseURL: nil)
    }
}
