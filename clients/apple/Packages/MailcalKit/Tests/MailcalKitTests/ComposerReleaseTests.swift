// An editor's web view exists only while the editor is on screen, and once.
//
// Each web view is a WebContent process of its own once it loads, so one built and kept for
// nothing is a process left running for the rest of the session: visible in Activity Monitor and
// nowhere in the UI. Nothing on screen tells a composer that built one web view from one that built
// five, which is why it is asserted here.
//
// The rule list each editor installs is the same kind of leak at a smaller size: every compile
// writes a new file and keeps it mapped, so it is compiled once and shared.

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

    @Test func theDropModifierDoesNotKeepTheEditorAlive() async {
        weak var released: RichComposerEditor?
        do {
            let editor = RichComposerEditor()
            released = editor
            let frame = NSRect(x: 0, y: 0, width: 400, height: 300)
            let host = NSHostingView(
                rootView: Color.clear.modifier(
                    ComposerDropModifier(
                        attachments: .constant([]),
                        droppedPictures: .constant([]),
                        composerError: .constant(nil),
                        editor: editor
                    )
                )
            )
            host.frame = frame
            let window = NSWindow(contentRect: frame, styleMask: [], backing: .buffered, defer: true)
            window.isReleasedWhenClosed = false
            window.contentView = host
            host.layoutSubtreeIfNeeded()
            // `onAppear` is what hands the editor its drop handler; let it run.
            await Task.yield()
            #expect(editor.acceptDroppedFiles != nil)
            window.contentView = nil
            window.close()
        }
        await Task.yield()
        #expect(released == nil)
    }

    @Test func everyEditorSharesOneCompiledRuleList() async {
        // Two at once, as two composers opened together ask, and one after both have their answer.
        async let first = compiledRuleList()
        async let second = compiledRuleList()
        let (together, alongside) = await (first, second)
        let later = await compiledRuleList()
        #expect(together != nil)
        #expect(together === alongside)
        #expect(together === later)
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
