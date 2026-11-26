// The editor's request channel (docs/composer-security.md, Gate 2): one named
// `WKScriptMessageHandler` on the composer's and the signature editor's web views. A message is
// parsed by the core (`parseComposerHostRequest`), which knows the whole vocabulary, so anything
// this file is handed is a request the product has a type for; an answer goes back through the
// script the core builds.

import MailcalBindings
import SwiftUI
import WebKit

/// The link dialog's starting point.
struct LinkDialogRequest: Identifiable, Equatable {
    let id: UInt64
    /// The selected words, or the existing link's.
    let text: String
    /// The existing link's target, empty when there is none.
    let address: String
    /// Whether the selection touches a link, so the dialog offers to remove it.
    let removable: Bool
}

/// The caret's top and bottom edges in the editor's page, as the framed mode reports them.
struct EditorCaret: Equatable {
    let top: CGFloat
    let bottom: CGFloat
}

@MainActor
final class ComposerHostChannel: NSObject, WKScriptMessageHandler {
    /// Called with each link request the core recognised.
    var onLink: (LinkDialogRequest) -> Void = { _ in }
    /// Called with the body's undo state each time it changes: whether there is a step to take
    /// back, and one to put back (`EditorUndoManager`).
    var onHistory: (_ undo: Bool, _ redo: Bool) -> Void = { _, _ in }
    /// Called in the editor's framed mode with where its caret is, in points from the top of the
    /// page (`docs/composer-layout.md`, rule 6).
    var onCaret: (EditorCaret) -> Void = { _ in }
    /// Called in the editor's framed mode with how far the message is scrolled inside its box, in
    /// points (`docs/composer-layout.md`, rule 2).
    var onScroll: (CGFloat) -> Void = { _ in }
    /// Called with every message the editor posts, before it is parsed: the core reads its
    /// reply-draft report from them (`composerReplyDraftReport`), which the parse leaves out.
    var onMessage: (String) -> Void = { _ in }
    /// Hands the core a message, answering whether it was the toolbar's sun or moon, which the core
    /// stores; the preference's Settings signal then names the page back to every editor
    /// (`ComposerPage.swift`).
    var choosesBackground: @MainActor (String) -> Bool = { _ in false }
    /// Runs a script the core wrote in the editor's document.
    var run: (String) -> Void = { _ in }
    /// Weak: the web view's content controller holds this channel, and the web view's owner holds
    /// the web view.
    private weak var webView: WKWebView?

    /// Registers the channel on `webView`. The editor asks for nothing until `announce()`.
    func install(on webView: WKWebView) {
        self.webView = webView
        run = { [weak webView] in webView?.evaluateJavaScript($0) }
        webView.configuration.userContentController.add(self, name: composerHostChannel())
    }

    /// Tells the editor which requests this host answers, and that it keeps the undo state. Run
    /// once the page has loaded.
    func announce() {
        webView?.evaluateJavaScript(composerHostRequestsWithHistoryScript())
    }

    func userContentController(
        _ userContentController: WKUserContentController,
        didReceive message: WKScriptMessage
    ) {
        guard message.name == composerHostChannel(), let body = message.body as? String else {
            return
        }
        receive(body)
    }

    /// One message the editor posted: to `onMessage`, every one, then to the core for the page's
    /// colours and its sun or moon, then to whichever handler its request has.
    func receive(_ body: String) {
        onMessage(body)
        if let answer = composerColoursAnswerScript(message: body) {
            run(answer)
            return
        }
        if choosesBackground(body) { return }
        guard let request = parseComposerHostRequest(message: body) else { return }
        switch request {
        case let .link(id, text, address, removable):
            onLink(LinkDialogRequest(id: id, text: text, address: address, removable: removable))
        case let .history(undo, redo):
            onHistory(undo, redo)
        case let .caret(top, bottom):
            onCaret(EditorCaret(top: CGFloat(top), bottom: CGFloat(bottom)))
        case let .scroll(px, _):
            onScroll(CGFloat(px))
        }
    }

    /// Delivers the link dialog's answer, with the keyboard back in the message it came from.
    func answer(_ request: LinkDialogRequest, with answer: ComposerLinkAnswer) {
        guard let webView else { return }
        #if os(macOS)
        webView.window?.makeFirstResponder(webView)
        #else
        webView.becomeFirstResponder()
        #endif
        webView.evaluateJavaScript(composerLinkAnswerScript(id: request.id, answer: answer))
    }
}

/// Presents the link dialog for whichever request is waiting, and answers it however it closes.
/// A sheet rather than anything inside the web view, so a small window or a short editor cannot
/// leave it half outside the frame it is drawn in.
struct ComposerLinkDialogModifier: ViewModifier {
    @Binding var request: LinkDialogRequest?
    let channel: ComposerHostChannel
    /// The request on screen and not yet answered, so a sheet swiped away still answers it.
    @State private var unanswered: LinkDialogRequest?

    func body(content: Content) -> some View {
        content.sheet(item: $request, onDismiss: cancelUnanswered) { request in
            LinkDialog(request: request) { answer in
                unanswered = nil
                self.request = nil
                channel.answer(request, with: answer)
            }
            .onAppear { unanswered = request }
        }
    }

    private func cancelUnanswered() {
        guard let request = unanswered else { return }
        unanswered = nil
        channel.answer(request, with: .cancel)
    }
}
