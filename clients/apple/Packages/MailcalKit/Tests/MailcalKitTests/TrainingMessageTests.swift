// What the training window answers for a selected row (docs/ai.md, "Training mode (debug
// builds)"): a message by itself, a conversation by its latest message. Debug builds on macOS
// only, like the window.

#if DEBUG && os(macOS)
import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct TrainingMessageTests {
    private let avatar = Avatar(
        initials: "M",
        light: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
        dark: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
        imagePath: nil
    )

    @Test func aMessageRowIsAnsweredByItself() {
        let row = SnapshotRow.flat(
            row: FlatRow(
                account: "acct-1", key: "m-1", subject: "Drawings", from: "Marc", avatar: avatar,
                date: "2026-07-20", unread: false, flagged: false, hasAttachment: false,
                preview: ""
            )
        )
        #expect(
            TrainingMessage(row)
                == TrainingMessage(accountId: "acct-1", key: "m-1", from: "Marc", subject: "Drawings")
        )
    }

    @Test func aConversationRowIsAnsweredByItsLatestMessage() {
        let row = SnapshotRow.thread(
            row: ThreadRow(
                account: "acct-1", threadId: "t-1", latestKey: "m-3", subject: "Drawings",
                latestFrom: "Marc", avatar: avatar, latestDate: "2026-07-20", messageCount: 3,
                unreadCount: 0, hasAttachment: false, preview: "", messages: []
            )
        )
        #expect(TrainingMessage(row).key == "m-3")
        #expect(TrainingMessage(row).from == "Marc")
    }
}
#endif
