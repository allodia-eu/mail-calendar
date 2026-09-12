// The host's half of the detached reading window (`docs/reading-window.md`): which window a
// message belongs in, what the registry does when the same row is opened twice, and that a window
// reads only its own slot.
//
// None of this needs a core: the registry is host bookkeeping, and the slot a view reads is a
// lookup. The core's half, that two readers hold two bodies and closing one frees it, is pinned in
// `crates/mailcal-app/src/tests_reading_windows.rs`.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite @MainActor struct ReadingWindowTests {
    private let avatar = Avatar(
        initials: "AL",
        light: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
        dark: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
        imagePath: nil
    )

    private func message(_ account: String, _ key: String, subject: String = "A subject")
        -> OpenedMessage
    {
        OpenedMessage(
            account: account, key: key, subject: subject,
            from: "Alice <alice@example.test>", avatar: avatar, date: "12 September 2026"
        )
    }

    private func body(_ key: String) -> ReadingSnapshot {
        ReadingSnapshot(
            key: key, from: "Alice <alice@example.test>", avatar: avatar,
            to: "", cc: "", bcc: "", html: "<p>Body of \(key)</p>", plain: nil,
            hasRemoteImages: false, loadError: false, attachments: [], invitation: nil,
            pending: false
        )
    }

    @Test func aWindowIsIdentifiedByItsMessageNotByWhenItWasOpened() {
        // What makes double-clicking a row twice reach the window that is already up: the id is
        // derived from the message, so the second open finds the first one's entry.
        let first = MailboxModel.readingWindowID(account: "acct-1", key: "m1")
        let again = MailboxModel.readingWindowID(account: "acct-1", key: "m1")
        #expect(first == again)
        // A provider key is unique only within its account, so the account has to be part of it:
        // without it, two accounts' `m1` would be one window.
        #expect(first != MailboxModel.readingWindowID(account: "acct-2", key: "m1"))
    }

    @Test func openingTheSameMessageTwiceKeepsOneWindowAndTheBodyItHas() {
        let model = MailboxModel()
        let opened = message("acct-1", "m1")
        model.openReadingWindow(opened)
        let id = MailboxModel.readingWindowID(account: "acct-1", key: "m1")
        model.readingWindows[id]?.body = body("m1")

        // The second double-click. It must not re-register the window: doing so would drop the
        // body back to nil and put a loading state over a message already on screen.
        model.openReadingWindow(opened)

        #expect(model.readingWindows.count == 1)
        #expect(model.readingWindows[id]?.body?.key == "m1")
    }

    @Test func eachWindowKeepsItsOwnMessage() {
        let model = MailboxModel()
        model.openReadingWindow(message("acct-1", "m1", subject: "The first"))
        model.openReadingWindow(message("acct-1", "m2", subject: "The second"))

        let first = MailboxModel.readingWindowID(account: "acct-1", key: "m1")
        let second = MailboxModel.readingWindowID(account: "acct-1", key: "m2")
        #expect(model.readingWindows[first]?.message.subject == "The first")
        #expect(model.readingWindows[second]?.message.subject == "The second")
    }

    @Test func closingOneWindowLeavesTheOthers() {
        let model = MailboxModel()
        model.openReadingWindow(message("acct-1", "m1"))
        model.openReadingWindow(message("acct-1", "m2"))

        model.closeReadingWindow(MailboxModel.readingWindowID(account: "acct-1", key: "m1"))

        #expect(model.readingWindows.count == 1)
        #expect(model.readingWindows[MailboxModel.readingWindowID(account: "acct-1", key: "m2")] != nil)
    }

    @Test func closingTheMainWindowTakesEveryReadingWindowWithIt() {
        // The rule the whole feature hangs off: one instance, and nothing left running behind it
        // (`docs/reading-window.md`).
        let model = MailboxModel()
        model.openReadingWindow(message("acct-1", "m1"))
        model.openReadingWindow(message("acct-2", "m9"))

        model.closeAllReadingWindows()

        #expect(model.readingWindows.isEmpty)
    }

    @Test func aWindowReadsItsOwnSlotAndNeverThePanes() {
        // The pane and a window are on different messages, which is the case the feature exists
        // for. Reading the wrong slot would show one message's body under the other's header.
        let model = MailboxModel()
        model.reading = body("m1")
        let opened = message("acct-1", "m2", subject: "The second")
        model.openReadingWindow(opened)
        let id = MailboxModel.readingWindowID(account: "acct-1", key: "m2")
        model.readingWindows[id]?.body = body("m2")

        let window = ReadingView(
            model: model, message: opened, source: .window(id),
            onReply: {}, onReplyAll: {}, onForward: {}, onArchive: {}, onDelete: {}
        )
        #expect(window.bodySnapshot?.key == "m2")

        // And the pane keeps reading the pane's, for the message the pane has open.
        let pane = ReadingView(
            model: model, message: message("acct-1", "m1"), source: .pane,
            onReply: {}, onReplyAll: {}, onForward: {}, onArchive: {}, onDelete: {}
        )
        #expect(pane.bodySnapshot?.key == "m1")
    }

    @Test func aWindowWhoseBodyHasNotArrivedShowsNothingRatherThanAnothersBody() {
        // A window is registered before its body lands. Until then its slot is empty, and the
        // guard on the key is what stops a snapshot for a different message being drawn under
        // this window's header.
        let model = MailboxModel()
        model.reading = body("m1")
        let opened = message("acct-1", "m2")
        model.openReadingWindow(opened)

        let window = ReadingView(
            model: model, message: opened,
            source: .window(MailboxModel.readingWindowID(account: "acct-1", key: "m2")),
            onReply: {}, onReplyAll: {}, onForward: {}, onArchive: {}, onDelete: {}
        )
        #expect(window.bodySnapshot == nil)
    }
}
