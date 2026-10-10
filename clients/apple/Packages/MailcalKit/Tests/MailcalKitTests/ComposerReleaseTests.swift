// An editor's web view exists only while the editor is on screen, and once.
//
// Each web view is a WebContent process of its own once it loads, so one built and kept for
// nothing is a process left running for the rest of the session: visible in Activity Monitor and
// nowhere in the UI. Nothing on screen tells a composer that built one web view from one that built
// five, which is why it is asserted here.
//
// The rule list each editor installs is the same kind of leak at a smaller size: every compile
// writes a new file and keeps it mapped, so it is compiled once and shared.

import MailcalBindings
import Testing

@testable import MailcalUI

#if os(macOS)
import AppKit
import SwiftUI
import WebKit

@MainActor
@Suite struct ComposerReleaseTests {

    @Test func buildingAnEditorBuildsNoWebView() {
        // SwiftUI runs a view's initialiser on every render of its parent and keeps the first
        // `@State` value, so each composer's initialiser builds an editor that is thrown away.
        // Building one has to cost nothing.
        #expect(RichComposerEditor().host.webView == nil)
        #expect(SignatureEditor().host.webView == nil)
    }

    @Test func mountingBuildsTheWebViewOnce() {
        let composer = RichComposerEditor()
        let first = composer.mount()
        #expect(composer.mount() === first)
        #expect(composer.host.webView === first)

        let signature = SignatureEditor()
        #expect(signature.mount() === signature.mount())
    }

    @Test func aComposerShowsOneWebViewHoweverOftenItsParentRenders() async {
        let renders = RenderCount()
        let frame = NSRect(x: 0, y: 0, width: 800, height: 600)
        let host = NSHostingView(rootView: RerenderingParent(renders: renders))
        host.frame = frame
        let window = NSWindow(contentRect: frame, styleMask: [], backing: .buffered, defer: true)
        window.isReleasedWhenClosed = false
        window.contentView = host
        for _ in 0..<5 {
            renders.value += 1
            host.layoutSubtreeIfNeeded()
            await Task.yield()
        }
        #expect(webViews(in: host).count == 1)
        window.contentView = nil
        window.close()
    }

    @Test(arguments: ComposerOnScreen.Kind.allCases)
    func aClosedComposerReleasesItsWebView(kind: ComposerOnScreen.Kind) async {
        // The whole composer, as the mailbox shows it: its own state behind every binding, the drop
        // handler, the link dialog and draft saving all attached, and two accounts so From is a
        // pop-up button. Anything among them that keeps the editor alive keeps this web view, and
        // its WebContent process, alive with it.
        weak var webView: WKWebView?
        let presence = Presence()
        let frame = NSRect(x: 0, y: 0, width: 800, height: 600)
        let host = NSHostingView(rootView: ComposerOnScreen(presence: presence, kind: kind))
        host.frame = frame
        let window = NSWindow(contentRect: frame, styleMask: [], backing: .buffered, defer: true)
        window.isReleasedWhenClosed = false
        window.contentView = host
        host.layoutSubtreeIfNeeded()
        await Task.yield()
        webView = webViews(in: host).first
        #expect(webView != nil)

        presence.shown = false
        host.layoutSubtreeIfNeeded()
        // Leaving runs the draft's close in a task of its own, which holds the editor until it has
        // read the body; give it the turns it needs rather than a wall-clock guess.
        for _ in 0..<200 where webView != nil {
            try? await Task.sleep(for: .milliseconds(10))
        }
        #expect(webView == nil)
        window.contentView = nil
        window.close()
    }

    @Test func theRuleListIsCompiledAndShared() async {
        let first = await compiledRuleList()
        let second = await compiledRuleList()
        #expect(first != nil)
        #expect(first === second)
    }

    private func compiledRuleList() async -> WKContentRuleList? {
        await withCheckedContinuation { done in
            ComposerRemoteBlock.ruleList { done.resume(returning: $0) }
        }
    }

    private func webViews(in view: NSView) -> [WKWebView] {
        view.subviews.flatMap { subview -> [WKWebView] in
            if let webView = subview as? WKWebView { return [webView] }
            return webViews(in: subview)
        }
    }
}

/// A count the parent below re-renders on.
@MainActor
@Observable
private final class RenderCount {
    var value = 0
}

/// Whether the composer below is on screen.
@MainActor
@Observable
final class Presence {
    var shown = true
}

/// The composer as a host shows it, with draft saving on, until `presence` says otherwise.
struct ComposerOnScreen: View {
    /// Which of the composer's optional controls it shows.
    enum Kind: CaseIterable {
        /// A new message: From, recipients, subject and the editor.
        case new
        /// A new message with a signature library, and one assigned to the sending account.
        case signed
        /// A reply carrying a quote, with the per-message quote-style picker.
        case reply
    }

    let presence: Presence
    let kind: Kind

    var body: some View {
        if presence.shown {
            RichComposeView(
                title: "New Message",
                mode: kind == .reply ? .reply : .new,
                accounts: [
                    AccountRow(id: "work", email: "work@example.test", name: "", expanded: true),
                    AccountRow(id: "home", email: "home@example.test", name: "", expanded: true),
                ],
                initialFrom: "work",
                initialTo: kind == .reply ? "sender@example.test" : "",
                quote: kind == .reply ? "<p>Earlier</p>" : nil,
                quoteStylePerMessage: kind == .reply,
                signatures: kind == .signed ? Self.signatures : nil,
                drafts: ComposerDrafts(
                    save: { _, _, _, _, _, _ in },
                    leave: { _, _, _, _, _, _ in },
                    discard: { _ in },
                    close: { _ in },
                    status: { _ in .idle },
                    isStored: { _ in false },
                    version: 0
                ),
                send: { _ in true },
                cancel: {}
            )
        }
    }

    private static let signature = SignatureBody(id: "s1", bodyHtml: "<p>Regards</p>", bodyPlain: "Regards")
    private static let signatures = ComposerSignatures(
        library: [SignatureRow(id: "s1", name: "Work")],
        forAccount: { _, _ in signature },
        byId: { _ in signature }
    )
}

/// A parent that builds the composer afresh on every render, as the mailbox does.
private struct RerenderingParent: View {
    let renders: RenderCount

    var body: some View {
        VStack {
            Text("\(renders.value)")
            RichComposeView(title: "New Message", send: { _ in true }, cancel: {})
        }
    }
}
#endif
