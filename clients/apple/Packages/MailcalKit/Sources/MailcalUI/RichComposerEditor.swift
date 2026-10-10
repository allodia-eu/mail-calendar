// The composer's editor: the shared `clients/composer/dist/editor.html` bundle in an `EditorHost`,
// and the seams the SwiftUI composer drives it through (seed the quote, seed/swap the signature,
// read the document back, focus the body).
//
// Split out of `RichComposerView.swift` so each file stays under the repo's 500-line rule: this
// file is the editor, that one is the SwiftUI composer. The hardened web view and when it exists
// are `EditorHost`'s.

import Foundation
import MailcalBindings
import SwiftUI
import WebKit

private enum RichComposerError: Error {
    case missingDocument
    case scriptFailed
}

@MainActor
@Observable
final class RichComposerEditor: NSObject {
    /// The web view and its gates. Nothing is built until the composer is on screen.
    @ObservationIgnored let host = EditorHost(
        fallbackHTML: "<!doctype html><html><body><script>window.composerDocument=function(){return JSON.stringify({blocks:[],attachments:[]});};</script></body></html>",
        // `EditorWebView` on both, for the drop the SwiftUI composer cannot reach
        // (EditorWebView.swift on macOS, EditorWebViewTouch.swift here). The macOS one also filters
        // its context menu down to the editing actions; iOS needs no such filter, an editable web
        // view already offers Cut/Copy/Paste in the system edit menu.
        makeView: { EditorWebView(frame: .zero, configuration: $0) }
    )
    /// The quoted-original seed (a `Block::Quote`-shaped JSON) to inject once the editor
    /// finishes loading, set for a reply/forward, `nil` for a new message. Applied once the bundle
    /// has loaded, because it loads asynchronously.
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
    /// The link dialog the editor is waiting on, if any.
    var linkRequest: LinkDialogRequest?
    /// Files dropped on the message itself, set by `ComposerDropModifier`, which owns what a dropped
    /// file becomes. The web view hands its drops here, so the handler can be set before or after
    /// the web view exists.
    @ObservationIgnored var acceptDroppedFiles: (([URL]) -> Void)?

    /// The document as it stood once the bundle had loaded and the quoted original (if any) had
    /// been seeded, the "nothing written yet" baseline `bodyChangedFromSeed()` compares against.
    /// `nil` until the editor is ready, at which point nothing can have been typed into it.
    private var seedDocument: String?

    /// The editor's request channel.
    var hostChannel: ComposerHostChannel { host.channel }

    override init() {
        super.init()
        host.channel.onLink = { [weak self] in self?.linkRequest = $0 }
        host.onLoad = { [weak self] in self?.seedOnLoad() }
    }

    /// The web view, for the representable: built on the first call, the same one after.
    func mount() -> WKWebView {
        host.mount { [weak self] view in
            (view as? EditorWebView)?.acceptDroppedFiles = { [weak self] urls in
                self?.acceptDroppedFiles?(urls)
            }
            #if os(iOS)
            // The document itself must never scroll: the page is a flex column whose `.editor`
            // scrolls inside itself, so anything that moves the *document* moves the toolbar
            // instead, and WebKit moves it on focus, scrolling the caret into view against a visual
            // viewport the keyboard accessory bar has just shortened. The result is a formatting
            // toolbar sliced through the middle the moment the message is tapped.
            //
            // Safe only because the host gives the web view a height that fits the toolbar plus the
            // editor's own `min-height` (`minimumEditorHeight`); below that the page would overflow
            // with nothing able to scroll it.
            view.scrollView.isScrollEnabled = false
            view.scrollView.bounces = false
            view.scrollView.contentInsetAdjustmentBehavior = .never
            // `isScrollEnabled` stops a *finger*, not WebKit: revealing the caret sets
            // `contentOffset` directly, and that is what moves the toolbar. Pinning the offset is
            // the part that actually holds.
            view.scrollView.delegate = self
            #endif
        }
    }

    func documentJSON(_ completion: @escaping (Result<String, Error>) -> Void) {
        host.evaluate("composerDocument()") { value, error in
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
    // any). Doing it here, not right after the load starts, guarantees the `window.setComposer*`
    // functions exist. Once both seeds are in, snapshot the document as the "nothing written yet"
    // baseline the discard prompt compares against, after them, so a reply with a
    // signature doesn't open already dirty.
    private func seedOnLoad() {
        // The chrome's own strings, before any content seed. They are independent of the seeds:
        // the placeholder lives on the editor element's dataset, which replacing the document does
        // not touch, but sending them first matches the other clients' open-time order.
        host.evaluate(ComposerLabels.script())
        host.channel.announce()
        // An agent-composed draft seeds a plain body instead of a quote. `setPlainText` assigns
        // it as TEXT, never markup (docs/composer-security.md, Gate 11), which matters more here
        // than anywhere else, because this body was written by a model that may itself have been
        // steered by a hostile message. It runs in the quote's slot, and for the same reason: it
        // assigns the whole body, so a signature seeded first would be wiped.
        if let body = pendingPlainBody, !body.isEmpty {
            host.evaluate("window.setPlainText(\(EditorHost.jsString(body)))") { [weak self] _, _ in
                self?.seedSignatureThenCapture()
            }
            return
        }
        guard let quote = pendingQuote else {
            seedSignatureThenCapture()
            return
        }
        host.evaluate("window.setComposerQuote(\(EditorHost.jsString(quote)))") { [weak self] _, _ in
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
        host.evaluate(
            "window.setComposerSignature(\(EditorHost.jsString(signature)))"
        ) { [weak self] _, _ in
            self?.captureSeed()
        }
    }

    /// Swaps the signature region in place, the auto-swap when the From account changes, and the
    /// per-message override picker. `nil` removes it ("None"). The user's typed text, their
    /// trimming of the quote and the caret are untouched; the editor only replaces that one region.
    func setSignature(_ json: String?) {
        let argument = json.map(EditorHost.jsString) ?? "null"
        host.evaluate("window.setComposerSignature(\(argument))")
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

    /// How many times the message has changed since the bundle loaded.
    ///
    /// Sampled rather than pushed: the page has no channel back to this host, so the bundle counts
    /// its own mutations and the host reads the count. What it is for is telling a composer still
    /// being typed in from one that has gone quiet, which is what decides when a draft is saved
    /// (`docs/drafts.md`). `0` while the bundle is still loading, which is also its starting
    /// value, so nothing reads as an edit before there is one.
    func revision() async -> Int {
        await withCheckedContinuation { continuation in
            host.evaluate("window.composerRevision()") { value, _ in
                continuation.resume(returning: (value as? NSNumber)?.intValue ?? 0)
            }
        }
    }

    /// Puts the caret in the message body, so a reply opens ready to type rather than making the
    /// user click into it first. On iOS this is also what raises the keyboard, which needs the web
    /// view to be first responder, not just the DOM element focused, hence both calls.
    func focusBody() {
        host.evaluate("window.focusComposerBody()")
        guard let webView = host.webView else { return }
        #if os(iOS)
        webView.becomeFirstResponder()
        #else
        webView.window?.makeFirstResponder(webView)
        #endif
    }

    /// Shows a picture at the caret. The shared editor records the inline attachment behind it and
    /// carries the bytes in the document, so the core can turn it into the `cid:` part the sent
    /// body points at; the same path a pasted screenshot takes, so a dropped and a pasted picture
    /// cannot behave differently.
    func insertImage(dataUrl: String, fileName: String) {
        let payload = ["data_url": dataUrl, "file_name": fileName]
        guard let data = try? JSONSerialization.data(withJSONObject: payload),
              let json = String(data: data, encoding: .utf8)
        else {
            return
        }
        host.evaluate("window.insertComposerImage(\(EditorHost.jsString(json)))")
    }

    /// Re-styles the quoted original in place without disturbing the user's typed message, the
    /// per-composer override of the persisted default.
    func setQuoteStyle(_ token: String) {
        host.evaluate("window.setComposerQuoteStyle(\(EditorHost.jsString(token)))")
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
