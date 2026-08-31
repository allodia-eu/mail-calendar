// The composer's editor host: the hardened `WKWebView` that loads the shared
// `clients/composer/dist/editor.html` bundle, and the seams the SwiftUI composer drives it through
// (seed the quote, seed/swap the signature, read the document back, focus the body).
//
// Split out of `RichComposerView.swift` so each file stays under the repo's 500-line rule: this
// file is the WebView plumbing and its security configuration, that one is the SwiftUI composer.
// The gates configured here are the Apple column of docs/composer-security.md.

import Foundation
import MailcalBindings
import SwiftUI
import UniformTypeIdentifiers
import WebKit

private enum RichComposerError: Error {
    case missingDocument
    case scriptFailed
}

@MainActor
final class RichComposerEditor: NSObject, ObservableObject, WKNavigationDelegate {
    let webView: WKWebView
    private var expectingInitialLoad = true
    /// The quoted-original seed (a `Block::Quote`-shaped JSON) to inject once the editor
    /// finishes loading, set for a reply/forward, `nil` for a new message. Applied in
    /// `didFinish` because the editor bundle loads asynchronously.
    var pendingQuote: String?
    /// The signature seed (a `Block::Signature`-shaped JSON) to inject once the editor finishes
    /// loading, `nil` when this account's slot is unassigned. Applied **after** the quote (the
    /// quote seed rewrites the whole document, which would wipe a signature injected first) and
    /// **before** the seed snapshot, so a composer that opened with a signature is not already
    /// "dirty" and does not prompt to discard on close.
    var pendingSignature: String?
    /// A plain-text body to seed once the bundle has loaded, an assistant's draft
    /// (`AgentComposerBridge`). Mutually exclusive with `pendingQuote` in practice: an agent
    /// draft is a new message. Seeded in the quote's place, and for the same reason before the
    /// signature: `setPlainText` assigns the whole body.
    var pendingPlainBody: String?
    /// Whether to put the caret in the message body once the editor has loaded. Set for a
    /// reply/forward, whose From/To/Subject are already filled in, so writing is the only thing
    /// left to do; a new message starts in its empty To field instead.
    var focusBodyOnLoad = false

    /// The document as it stood once the bundle had loaded and the quoted original (if any) had
    /// been seeded, the "nothing written yet" baseline `bodyChangedFromSeed()` compares against.
    /// `nil` until the editor is ready, at which point nothing can have been typed into it.
    private var seedDocument: String?

    override init() {
        let configuration = WKWebViewConfiguration()
        let preferences = WKWebpagePreferences()
        preferences.allowsContentJavaScript = true
        configuration.defaultWebpagePreferences = preferences
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        configuration.websiteDataStore = .nonPersistent()
        webView = WKWebView(frame: .zero, configuration: configuration)
        super.init()
        webView.navigationDelegate = self
        webView.allowsBackForwardNavigationGestures = false
        #if os(iOS)
        // The document itself must never scroll: the page is a flex column whose `.editor` scrolls
        // inside itself, so anything that moves the *document* moves the toolbar instead, and
        // WebKit moves it on focus, scrolling the caret into view against a visual viewport the
        // keyboard accessory bar has just shortened. The result is a formatting toolbar sliced
        // through the middle the moment the message is tapped.
        //
        // Safe only because the host gives the web view a height that fits the toolbar plus the
        // editor's own `min-height` (`minimumEditorHeight`); below that the page would overflow
        // with nothing able to scroll it.
        webView.scrollView.isScrollEnabled = false
        webView.scrollView.bounces = false
        webView.scrollView.contentInsetAdjustmentBehavior = .never
        // `isScrollEnabled` stops a *finger*, not WebKit: revealing the caret sets `contentOffset`
        // directly, and that is what moves the toolbar. Pinning the offset is the part that
        // actually holds.
        webView.scrollView.delegate = self
        #endif
        installRemoteBlockThenLoad()
    }

    // Compiles a content rule list that blocks every remote (http/https) sub-resource load
    // the Apple half of the composer's "no network egress" gate (docs/composer-security.md),
    // a native barrier behind the bundle's CSP, matching Android's `shouldInterceptRequest`
    // and Windows' `WebResourceRequested` 403. If compilation fails the CSP still blocks
    // remote loads, so we load the editor anyway rather than leave the composer blank.
    private func installRemoteBlockThenLoad() {
        WKContentRuleListStore.default().compileContentRuleList(
            forIdentifier: "composer-block-remote",
            encodedContentRuleList: Self.blockRemoteRuleList
        ) { [weak self] ruleList, _ in
            guard let self else { return }
            if let ruleList {
                self.webView.configuration.userContentController.add(ruleList)
            }
            self.loadEditor()
        }
    }

    private static let blockRemoteRuleList = """
        [{"trigger":{"url-filter":"^https?://"},"action":{"type":"block"}}]
        """

    func documentJSON(_ completion: @escaping (Result<String, Error>) -> Void) {
        webView.evaluateJavaScript("composerDocument()") { value, error in
            if error != nil {
                completion(.failure(RichComposerError.scriptFailed))
                return
            }
            guard let document = value as? String, !document.isEmpty else {
                completion(.failure(RichComposerError.missingDocument))
                return
            }
            completion(.success(document))
        }
    }

    // The editor bundle has finished loading; seed the quoted original and the signature now (if
    // any). Doing it here, not right after `loadHTMLString`, guarantees the `window.setComposer*`
    // functions exist. Once both seeds are in, snapshot the document as the "nothing written yet"
    // baseline the discard prompt compares against, after them, so a reply with a
    // signature doesn't open already dirty.
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        // The chrome's own strings, before any content seed. They are independent of the seeds:
        // the placeholder lives on the editor element's dataset, which replacing the document does
        // not touch, but sending them first matches the other clients' open-time order.
        webView.evaluateJavaScript(ComposerLabels.script())
        // An agent-composed draft seeds a plain body instead of a quote. `setPlainText` assigns
        // it as TEXT, never markup (docs/composer-security.md, Gate 11), which matters more here
        // than anywhere else, because this body was written by a model that may itself have been
        // steered by a hostile message. It runs in the quote's slot, and for the same reason: it
        // assigns the whole body, so a signature seeded first would be wiped.
        if let body = pendingPlainBody, !body.isEmpty {
            webView.evaluateJavaScript("window.setPlainText(\(Self.jsString(body)))") { [weak self] _, _ in
                self?.seedSignatureThenCapture()
            }
            return
        }
        guard let quote = pendingQuote else {
            seedSignatureThenCapture()
            return
        }
        webView.evaluateJavaScript("window.setComposerQuote(\(Self.jsString(quote)))") { [weak self] _, _ in
            self?.seedSignatureThenCapture()
        }
    }

    /// Injects the opening signature (if the account has one for this mode), then snapshots the
    /// seed. Ordered strictly after the quote: `setComposerQuote` replaces the document wholesale.
    private func seedSignatureThenCapture() {
        guard let signature = pendingSignature else {
            captureSeed()
            return
        }
        webView.evaluateJavaScript(
            "window.setComposerSignature(\(Self.jsString(signature)))"
        ) { [weak self] _, _ in
            self?.captureSeed()
        }
    }

    /// Swaps the signature region in place, the auto-swap when the From account changes, and the
    /// per-message override picker. `nil` removes it ("None"). The user's typed text, their
    /// trimming of the quote and the caret are untouched; the editor only replaces that one region.
    func setSignature(_ json: String?) {
        let argument = json.map(Self.jsString) ?? "null"
        webView.evaluateJavaScript("window.setComposerSignature(\(argument))")
    }

    // Snapshot the document as the "nothing written yet" baseline, then, for a reply/forward:
    // put the caret in the body. Focusing after the snapshot, never before: moving the caret must
    // not be mistaken for the user having typed, or a reply would open already dirty and prompt to
    // discard on close.
    private func captureSeed() {
        documentJSON { [weak self] result in
            if case let .success(document) = result {
                self?.seedDocument = document
            }
            if self?.focusBodyOnLoad == true {
                self?.focusBody()
            }
        }
    }

    /// Whether the editor document differs from the seed it opened with, i.e. the user has written
    /// something, or restyled the quote. Until the seed is captured the bundle is still loading, so
    /// nothing can have been typed into it: not dirty. A read that fails is likewise not dirty; the
    /// header fields are checked separately and are the common case.
    func bodyChangedFromSeed() async -> Bool {
        guard let seed = seedDocument else { return false }
        return await withCheckedContinuation { continuation in
            documentJSON { result in
                switch result {
                case let .success(document):
                    continuation.resume(returning: document != seed)
                case .failure:
                    continuation.resume(returning: false)
                }
            }
        }
    }

    /// Puts the caret in the message body, so a reply opens ready to type rather than making the
    /// user click into it first. On iOS this is also what raises the keyboard, which needs the web
    /// view to be first responder, not just the DOM element focused, hence both calls.
    func focusBody() {
        webView.evaluateJavaScript("window.focusComposerBody()")
        #if os(iOS)
        webView.becomeFirstResponder()
        #else
        webView.window?.makeFirstResponder(webView)
        #endif
    }

    /// Re-styles the quoted original in place without disturbing the user's typed message, the
    /// per-composer override of the persisted default.
    func setQuoteStyle(_ token: String) {
        webView.evaluateJavaScript("window.setComposerQuoteStyle(\(Self.jsString(token)))")
    }

    /// Encodes `value` as a JavaScript string literal (quoted + escaped) so it can be passed
    /// safely into an `evaluateJavaScript` call without breaking out of the argument.
    private static func jsString(_ value: String) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: value, options: .fragmentsAllowed),
              let literal = String(data: data, encoding: .utf8)
        else {
            return "\"\""
        }
        return literal
    }

    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        if expectingInitialLoad {
            expectingInitialLoad = false
            decisionHandler(.allow)
        } else {
            decisionHandler(.cancel)
        }
    }

    private func loadEditor() {
        let asset = RichComposerAsset.load()
        webView.loadHTMLString(asset.html, baseURL: asset.baseURL)
    }
}

private struct RichComposerAsset {
    let html: String
    let baseURL: URL?

    static func load() -> RichComposerAsset {
        // The editor bundle ships as an SPM resource (Bundle.module); Bundle.main covers a host
        // that copies it into the app bundle instead.
        for bundle in [Bundle.module, Bundle.main] {
            if let bundleURL = bundle.url(
                forResource: "editor",
                withExtension: "html",
                subdirectory: "composer"
            ), let html = try? String(contentsOf: bundleURL, encoding: .utf8) {
                return RichComposerAsset(html: html, baseURL: bundleURL.deletingLastPathComponent())
            }
        }

        let sourceURL = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("composer/editor.html")
        if let html = try? String(contentsOf: sourceURL, encoding: .utf8) {
            return RichComposerAsset(html: html, baseURL: sourceURL.deletingLastPathComponent())
        }

        return RichComposerAsset(
            html: "<!doctype html><html><body><script>window.composerDocument=function(){return JSON.stringify({blocks:[],attachments:[]});};</script></body></html>",
            baseURL: nil
        )
    }
}

#if os(iOS)
extension RichComposerEditor: UIScrollViewDelegate {
    /// Keeps the editor document at its origin.
    ///
    /// The page is a flex column that scrolls **inside** `.editor`; the document around it is
    /// furniture and must not move, or the formatting toolbar slides off the top of the web view
    /// the moment the message is tapped.
    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        if scrollView.contentOffset != .zero {
            scrollView.contentOffset = .zero
        }
    }
}
#endif
