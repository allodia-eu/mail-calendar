// A closed composer gives its editor back.
//
// Each editor is a `WKWebView`, and each web view a WebContent process of its own, so a composer
// kept alive after it closes is a process left running for the rest of the session: one per "New
// message" opened, visible in Activity Monitor and nowhere in the UI. Nothing on screen tells a
// leaked editor from a released one, which is why it is asserted here.
//
// The rule list each editor installs is the same kind of leak at a smaller size: every compile
// writes a new file and keeps it mapped, one per composer opened, so it is compiled once and shared.

import Testing

@testable import MailcalUI

#if os(macOS)
import AppKit
import SwiftUI
import WebKit

@MainActor
@Suite struct ComposerReleaseTests {

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
            // `onAppear` is what hands the web view its drop handler; let it run.
            await Task.yield()
            #expect((editor.webView as? EditorWebView)?.acceptDroppedFiles != nil)
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
}
#endif
