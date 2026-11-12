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
        // (EditorWebView.swift on macOS, EditorWebViewTouch.swift here). The touch one also takes a
        // paste, to hand the bundle the clipboard's HTML as written. The macOS one also filters
        // its context menu down to the editing actions; iOS needs no such filter, an editable web
        // view already offers Cut/Copy/Paste in the system edit menu.
        makeView: { EditorWebView(frame: .zero, configuration: $0) }
    )
    /// The quoted-original seed (a `Block::Quote`-shaped JSON) to inject once the editor
    /// finishes loading: set for a fresh reply or forward, `nil` otherwise. Applied once the
    /// bundle has loaded, because it loads asynchronously.
    var pendingQuote: String?
    /// The signature seed (a `Block::Signature`-shaped JSON) to inject once the editor finishes
    /// loading, `nil` when this account's slot is unassigned. Applied **after** the quote (the
    /// quote seed rewrites the whole document, which would wipe a signature injected first) and
    /// **before** the seed snapshot, so a composer that opened with a signature is not already
    /// "dirty" and does not prompt to discard on close.
    var pendingSignature: String?
    /// A plain-text body to seed once the bundle has loaded: a link's, a share's or an assistant's.
    /// Seeded only when there is neither a stored body nor a quote (a prefill's text above a quote
    /// rides in the quote seed), and for the same reason before the signature: `setPlainText`
    /// assigns the whole body.
    var pendingPlainBody: String?
    /// The HTML of a message this composer reopens (a resumed draft, or one moved back out of the
    /// Outbox), seeded ahead of everything else through `setComposerBody`, which reads it back
    /// into the editor's document; `pendingPlainBody` then rides beside it as that message's text.
    var pendingStoredHTML: String?
    /// The attribution a reopened quote is completed with once the signature is in, so the style
    /// picker switches it to a full one (`completeComposerQuoteAttribution`).
    var pendingQuoteAttribution: String?
    /// The document a composer moved from another view left with (`MovedComposer.documentJson`),
    /// seeded through `setComposerDocument` in place of every other body seed.
    var pendingDocument: String?
    /// The style the reopened message's quote was saved in, read back from the document once it
    /// is seeded, for the style picker to open on. `nil` for anything that is not reopened.
    private(set) var storedQuoteStyle: QuoteStyleKind?
    /// Whether to put the caret in the message body once the editor has loaded. Set for a
    /// reply/forward, whose From/To/Subject are already filled in, so writing is the only thing
    /// left to do; a new message starts in its empty To field instead.
    var focusBodyOnLoad = false
    /// The link dialog the editor is waiting on, if any.
    var linkRequest: LinkDialogRequest?
    /// Where the caret last was, for the composer's column to keep in view.
    @ObservationIgnored var caret: EditorCaret?
    /// How many caret reports have arrived. The column follows each one, including one at the same
    /// place as the last: the page resends it on every edit, because the column may have moved since.
    var caretReports = 0
    /// How far the message is scrolled inside its box, as the framed mode last reported it.
    @ObservationIgnored var messageOffset: CGFloat = 0
    /// The scroll offset of the column holding the editor and the furthest it scrolls, as the
    /// column last reported them.
    @ObservationIgnored var columnOffset: CGFloat = 0
    @ObservationIgnored var columnEnd: CGFloat = 0

    /// Whether a vertical step over the editor goes to the column rather than to the message
    /// (`docs/composer-layout.md`, rule 2): the core's answer, from where both last were.
    func columnTakesScroll(towardsEnd: Bool) -> Bool {
        composerScroller(
            columnOffsetPx: columnOffset,
            columnEndPx: columnEnd,
            messageOffsetPx: messageOffset,
            towardsEnd: towardsEnd
        ) == .column
    }
    /// The height of the header drawn over the top of the page on a touch screen, which the page
    /// keeps clear above the message.
    @ObservationIgnored private var topInset: CGFloat = 0

    /// Keeps `height` clear at the top of the page for the header over it. Sent again on load, so
    /// it may be set before the page is there.
    func setTopInset(_ height: CGFloat) {
        guard abs(height - topInset) >= 0.5 else { return }
        topInset = height
        host.evaluate(Self.topInsetScript(height))
    }

    private static func topInsetScript(_ height: CGFloat) -> String {
        "window.setComposerTopInset(\(Double(height)))"
    }

    #if os(iOS)
    /// Keeps the page out of sight until the host has switched on its touch layout.
    ///
    /// Until then the page draws the layout a desktop uses, its toolbar at the top running off the
    /// screen and its message under the native header, for as long as the bundle takes to load.
    /// Run before the document is parsed, so its first frame is already hidden; the header stays in
    /// view and takes its taps meanwhile.
    private static let hiddenUntilTouchLayout = WKUserScript(
        source: """
        (function () {
          var style = document.createElement("style");
          style.textContent = "html:not(.native-chrome) body { visibility: hidden; }";
          document.documentElement.appendChild(style);
        })();
        """,
        injectionTime: .atDocumentStart,
        forMainFrameOnly: true
    )
    #endif

    /// Files dropped on the message itself, set by `ComposerDropModifier`, which owns what a dropped
    /// file becomes. The web view hands its drops here, so the handler can be set before or after
    /// the web view exists.
    @ObservationIgnored var acceptDroppedFiles: (([URL]) -> Void)?

    /// The document as it stood once the bundle had loaded and the quoted original (if any) had
    /// been seeded, the "nothing written yet" baseline `bodyChangedFromSeed()` compares against.
    /// `nil` until the editor is ready, at which point nothing can have been typed into it.
    private var seedDocument: String?
    /// The editor's change count as the seeds left it, the baseline the autosave compares every
    /// sample against (`EditorRevisionWatch`). `nil` until every seed is in; `-1` when the count
    /// could not be read then.
    private(set) var seedRevision: Int?

    /// The editor's request channel.
    var hostChannel: ComposerHostChannel { host.channel }

    override init() {
        super.init()
        host.channel.onLink = { [weak self] in self?.linkRequest = $0 }
        host.channel.onCaret = { [weak self] in
            self?.caret = $0
            self?.caretReports &+= 1
        }
        host.channel.onScroll = { [weak self] in self?.messageOffset = $0 }
        host.onLoad = { [weak self] in self?.seedOnLoad() }
    }

    /// The web view, for the representable: built on the first call, the same one after.
    func mount() -> WKWebView {
        host.mount { view in
            (view as? EditorWebView)?.acceptDroppedFiles = { [weak self] urls in
                self?.acceptDroppedFiles?(urls)
            }
            #if os(macOS)
            (view as? EditorWebView)?.columnTakesScroll = { [weak self] towardsEnd in
                self?.columnTakesScroll(towardsEnd: towardsEnd) ?? false
            }
            // The framed page has no background, so the toolbar sits on the column's own colour
            // (`docs/composer-layout.md`, rule 7); a web view that painted white under it would put
            // the strip back. There is no public switch on macOS: key-value coding reaches
            // WebKit's own `_setDrawsBackground:`, asked for first so a WebKit without it is left
            // as it is rather than taken down.
            if view.responds(to: NSSelectorFromString("_setDrawsBackground:")) {
                view.setValue(false, forKey: "drawsBackground")
            }
            #endif
            #if os(iOS)
            // The page is the composer's one scroll on a touch screen, with the header drawn inside
            // it (`TouchComposerChrome`); the frame already ends at the keyboard, so WebKit is
            // given no insets of its own to add.
            view.scrollView.contentInsetAdjustmentBehavior = .never
            view.scrollView.keyboardDismissMode = .interactive
            view.configuration.userContentController.addUserScript(Self.hiddenUntilTouchLayout)
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

    // The editor bundle has finished loading; seed the body, the signature and a reopened quote's
    // attribution now, in that order. Doing it here, not right after the load starts, guarantees
    // the `window.setComposer*` functions exist. Once every seed is in, snapshot the document as
    // the "nothing written yet" baseline the discard prompt compares against, after them, so a
    // reply with a signature doesn't open already dirty.
    private func seedOnLoad() {
        // The chrome's own strings, before any content seed. They are independent of the seeds:
        // the placeholder lives on the editor element's dataset, which replacing the document does
        // not touch, but sending them first matches the other clients' open-time order.
        host.evaluate(ComposerLabels.script())
        #if os(iOS)
        // A touch screen's layout: the page scrolls, the header sits in the room the top inset
        // keeps, and the toolbar rides on the keyboard (`docs/composer-layout.md`, rule 7).
        // The inset first: the page shows itself once the layout is on (`hiddenUntilTouchLayout`),
        // and its first frame should already leave the header its room.
        if topInset > 0 { host.evaluate(Self.topInsetScript(topInset)) }
        host.evaluate("window.useNativeComposerChrome()")
        #else
        // A desktop column's layout: the header scrolls away, then the message in its own box under
        // the toolbar. Before the announce, which the page answers with its caret and its scroll.
        host.evaluate("window.useFramedComposer()")
        #endif
        host.channel.announce()
        // One body seed, whichever way the composer was reached. A plain body is assigned as
        // TEXT, never markup (docs/composer-security.md, Gate 11), which matters most for an
        // assistant's draft, written by a model that may itself have been steered by a hostile
        // message. Each seed assigns the whole body, so a signature seeded first would be wiped.
        let seed = pendingDocument.map { "window.setComposerDocument(\(EditorHost.jsString($0)))" }
            ?? Self.openingSeedScript(html: pendingStoredHTML, quote: pendingQuote, text: pendingPlainBody)
        guard let body = seed else {
            seedSignatureThenCapture()
            return
        }
        host.evaluate(body) { [weak self] _, _ in
            self?.seedSignatureThenCapture()
        }
    }

    /// Injects the opening signature (if the account has one for this mode), then completes a
    /// reopened quote's attribution. Ordered strictly after the body: each body seed replaces the
    /// document wholesale.
    private func seedSignatureThenCapture() {
        guard let signature = pendingSignature else {
            completeAttributionThenCapture()
            return
        }
        host.evaluate(
            "window.setComposerSignature(\(EditorHost.jsString(signature)))"
        ) { [weak self] _, _ in
            self?.completeAttributionThenCapture()
        }
    }

    /// Fills the half of a reopened quote's attribution its stored HTML does not carry, then
    /// snapshots the seed, so the completion is not mistaken for the user having written.
    private func completeAttributionThenCapture() {
        guard let attribution = pendingQuoteAttribution else {
            captureSeed()
            return
        }
        host.evaluate(
            "window.completeComposerQuoteAttribution(\(EditorHost.jsString(attribution)))"
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

    // Snapshot the document and the change count as the "nothing written yet" baselines, then, for
    // a reply/forward: put the caret in the body. Focusing after both, never before: moving the
    // caret must not be mistaken for the user having typed, or a reply would open already dirty,
    // prompt to discard on close and be saved by the idle timer.
    private func captureSeed() {
        documentJSON { [weak self] result in
            if case let .success(document) = result {
                self?.seedDocument = document
                if self?.pendingStoredHTML != nil || self?.pendingDocument != nil {
                    self?.storedQuoteStyle = ComposeOpeningSeed.quoteStyle(ofDocument: document)
                }
            }
            self?.host.evaluate("window.composerRevision()") { value, _ in
                self?.seedRevision = (value as? NSNumber)?.intValue ?? -1
                if self?.focusBodyOnLoad == true {
                    self?.focusBody()
                }
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
    /// (`docs/drafts.md`). `-1` when it cannot be read, which `EditorRevisionWatch` ignores.
    func revision() async -> Int {
        await withCheckedContinuation { continuation in
            host.evaluate("window.composerRevision()") { value, _ in
                continuation.resume(returning: (value as? NSNumber)?.intValue ?? -1)
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

    /// The call that seeds the body, in the one order every client keeps: a reopened message's
    /// HTML through `setComposerBody`, else a fresh reply's or forward's quote through
    /// `setComposerQuote`, else a text through `setPlainText`, else nothing.
    nonisolated static func openingSeedScript(html: String?, quote: String?, text: String?) -> String? {
        let stored = html.map { !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty } ?? false
        if !stored, let quote {
            return "window.setComposerQuote(\(EditorHost.jsString(quote)))"
        }
        return bodySeedScript(html: html, text: text)
    }

    /// The call that seeds a stored or plain body: a reopened message's HTML through
    /// `setComposerBody`, else a text through `setPlainText`, else nothing. Everything is passed
    /// as one JSON string the editor parses, never spliced into the script, so no body can close
    /// the call and run.
    nonisolated static func bodySeedScript(html: String?, text: String?) -> String? {
        if let html, !html.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
           let data = try? JSONSerialization.data(withJSONObject: ["html": html, "text": text ?? ""]),
           let seed = String(data: data, encoding: .utf8) {
            return "window.setComposerBody(\(EditorHost.jsString(seed)))"
        }
        guard let text, !text.isEmpty else { return nil }
        return "window.setPlainText(\(EditorHost.jsString(text)))"
    }
}
