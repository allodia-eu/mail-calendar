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
import MailcalBindings
import SwiftUI
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
    private var loaded = false
    /// The app's scheme and the message's page as last named, `nil` until a representable has
    /// named them.
    private var scheme: (dark: Bool, page: MessageScheme)?

    /// - Parameters:
    ///   - makeView: builds the web view from the hardened configuration, for a host that needs a
    ///     subclass of its own (the composer's `EditorWebView`). A `HistoryWebView`, or the
    ///     platform's Undo and Redo read WebKit's stack rather than the editor's.
    ///   - fallbackHTML: the document loaded when the bundle cannot be found, which defines the
    ///     functions its host reads back so a read answers empty rather than failing.
    init(
        fallbackHTML: String,
        makeView: @escaping (WKWebViewConfiguration) -> WKWebView = {
            HistoryWebView(frame: .zero, configuration: $0)
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
        Self.drawNoBackground(view)
        channel.install(on: view)
        if let history = (view as? HistoryWebView)?.history {
            channel.onHistory = { [weak history] undo, redo in history?.update(undo: undo, redo: redo) }
            history.step = { [weak self] step in
                self?.evaluate("window.stepComposerHistory(\(Self.jsString(step)))")
            }
        }
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
            // Both are named before the editor's document runs, so its first frame is drawn on
            // them rather than on a light page the load's own naming then repaints.
            if let scheme = self.scheme {
                view.configuration.userContentController.addUserScript(WKUserScript(
                    source: Self.preloadScript(dark: scheme.dark, page: scheme.page),
                    injectionTime: .atDocumentStart,
                    forMainFrameOnly: true,
                    in: .page
                ))
            }
            let asset = EditorAsset.load(fallbackHTML: self.fallbackHTML)
            view.loadHTMLString(asset.html, baseURL: asset.baseURL)
        }
        return view
    }

    /// Leaves the web view no background of its own, so the app's surface shows wherever the page
    /// has not painted: until the document's first paint, which a web view otherwise fills white
    /// in a dark app, and under the composer's framed toolbar, which sits on the column's own
    /// colour (`docs/composer-layout.md`, rule 7).
    private static func drawNoBackground(_ view: WKWebView) {
        #if os(macOS)
        // No public switch on macOS: key-value coding reaches WebKit's own `_setDrawsBackground:`,
        // asked for first so a WebKit without it is left as it is rather than taken down.
        if view.responds(to: NSSelectorFromString("_setDrawsBackground:")) {
            view.setValue(false, forKey: "drawsBackground")
        }
        #else
        view.isOpaque = false
        view.backgroundColor = .clear
        view.scrollView.backgroundColor = .clear
        #endif
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
    nonisolated static func jsString(_ value: String) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: value, options: .fragmentsAllowed),
              let literal = String(data: data, encoding: .utf8)
        else {
            return "\"\""
        }
        return literal
    }

    /// Follows the representable's environment: the app's light or dark, which the editor's chrome
    /// follows (`clients/composer/src/scheme.ts`), and the page its message is written on
    /// (`ComposerPage.swift`). Named before the document runs when called before `mount()`, sent
    /// now if the bundle is up, else as it loads. SwiftUI's colour scheme carries a Light or Dark
    /// chosen in the app.
    func follow(_ environment: EnvironmentValues) {
        let source = environment.composerPage
        channel.choosesBackground = source.choosesBackground
        let dark = environment.colorScheme == .dark
        let page = MessageScheme.of(appDark: dark, darkMessages: source.darkMessages)
        if let scheme, scheme.dark == dark, scheme.page == page { return }
        scheme = (dark, page)
        if loaded {
            evaluate(Self.schemeScript(dark: dark, page: page))
        }
    }

    /// The script naming the app's scheme to the chrome and the page to the message.
    nonisolated static func schemeScript(dark: Bool, page: MessageScheme) -> String {
        "window.setComposerScheme(\"\(dark ? "dark" : "light")\");" + composerPageScript(scheme: page)
    }

    /// The same as a script the web view runs before the editor's document does.
    nonisolated static func preloadScript(dark: Bool, page: MessageScheme) -> String {
        "window.composerScheme = \"\(dark ? "dark" : "light")\";" + composerPagePreloadScript(scheme: page)
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        loaded = true
        if let scheme {
            evaluate(Self.schemeScript(dark: scheme.dark, page: scheme.page))
        }
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
