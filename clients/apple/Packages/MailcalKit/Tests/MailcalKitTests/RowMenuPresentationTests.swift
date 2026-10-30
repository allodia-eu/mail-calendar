// How the Apple list reads the core's row menu, and the rule for which conversation stays
// expanded (`docs/row-actions.md`), without a window. Both fail silently in the running app: a bar
// naming the half of a toggle that changes nothing does nothing when clicked, and a conversation
// left open after the user moved on keeps its messages in a list they are no longer reading.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct RowMenuPresentationTests {
    private func avatar() -> Avatar {
        Avatar(
            initials: "S",
            light: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
            dark: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
            imagePath: nil
        )
    }

    private func message(_ key: String) -> ThreadMessage {
        ThreadMessage(
            account: "acct-1", key: key, from: "sender", avatar: avatar(), date: "2026-10-30",
            preview: "", unread: false, outgoing: false, draft: false, hasAttachment: false
        )
    }

    private func conversation(_ id: String) -> ThreadRow {
        ThreadRow(
            account: "acct-1", threadId: id, latestKey: "\(id)-1", subject: "Conversation",
            latestFrom: "sender", avatar: avatar(), latestDate: "2026-10-30", messageCount: 2,
            unreadCount: 0, hasAttachment: false, preview: "",
            messages: [message("\(id)-2"), message("\(id)-1")]
        )
    }

    private func flat(_ key: String) -> SnapshotRow {
        .flat(row: FlatRow(
            account: "acct-1", key: key, subject: "Subject", from: "sender", avatar: avatar(),
            date: "2026-10-30", unread: false, flagged: false, hasAttachment: false, preview: ""
        ))
    }

    private func item(_ action: RowAction, enabled: Bool = true) -> RowMenuItem {
        RowMenuItem(action: action, enabled: enabled, target: nil)
    }

    // MARK: The bar

    @Test func theBarOffersTheHalfOfEachToggleTheCoreNamed() {
        let menu = RowMenu(sections: [
            RowMenuSection(items: [item(.markUnread), item(.unflag)]),
            RowMenuSection(items: [item(.archive, enabled: false), item(.delete)]),
        ])
        let bar = SelectionBarActions(menu)
        #expect(bar.read.action == .markUnread)
        #expect(bar.flag.action == .unflag)
        #expect(!bar.archive.enabled, "a move that would take nothing is disabled on the bar too")
        #expect(bar.delete.enabled)
    }

    /// The desktop bar stands with nothing picked, and the core answers no menu over no rows.
    /// "Mark as unread" over nothing is the wrong sentence to leave on screen.
    @Test func anEmptySelectionNamesTheAffirmativeActionsDisabled() {
        let bar = SelectionBarActions(RowMenu(sections: []))
        #expect(bar.read.action == .markRead)
        #expect(bar.flag.action == .flag)
        let buttons = [bar.read, bar.flag, bar.archive, bar.delete, bar.permanentlyDelete]
        #expect(buttons.allSatisfy { !$0.enabled })
    }

    @Test func onlyAnItemWithoutAMessageRunsTheBatch() {
        #expect(RowAction.archive.bulkAction == .archive)
        #expect(RowAction.unflag.bulkAction == .unflag)
        for action in [RowAction.open, .reply, .markAsSpam, .moveToFolder] {
            #expect(action.bulkAction == nil, "\(action) has no batch form")
        }
    }

    // MARK: Expansion

    @Test func leavingAConversationFoldsItAway() {
        let open = conversation("t1")
        let rows: [SnapshotRow] = [.thread(row: open), flat("m1")]
        let expanded: Set = [ConversationExpansion.key(open)]

        let elsewhere = ConversationExpansion.kept(
            expanded, selected: [SelectionKey(flat("m1"))], rows: rows
        )
        #expect(elsewhere.isEmpty)

        let inside = ConversationExpansion.kept(
            expanded, selected: [SelectionKey(open.messages[1])], rows: rows
        )
        #expect(inside == expanded, "one of its own messages keeps it open")

        let itself = ConversationExpansion.kept(
            expanded, selected: [SelectionKey(.thread(row: open))], rows: rows
        )
        #expect(itself == expanded)
    }

    /// Several rows picked, or none, is not the user leaving a conversation: adding one to a set
    /// expands nothing and folds nothing, and an archive that emptied the selection is not a move.
    @Test func aSelectionOfSeveralOrNoneChangesNothing() {
        let open = conversation("t1")
        let rows: [SnapshotRow] = [.thread(row: open), flat("m1"), flat("m2")]
        let expanded: Set = [ConversationExpansion.key(open)]

        let several: Set = [SelectionKey(flat("m1")), SelectionKey(flat("m2"))]
        #expect(ConversationExpansion.kept(expanded, selected: several, rows: rows) == expanded)
        #expect(ConversationExpansion.kept(expanded, selected: [], rows: rows) == expanded)
    }

    @Test func onlyTheConversationHoldingTheSelectionStaysOpen() {
        let first = conversation("t1")
        let second = conversation("t2")
        let rows: [SnapshotRow] = [.thread(row: first), .thread(row: second)]
        let both: Set = [ConversationExpansion.key(first), ConversationExpansion.key(second)]

        let kept = ConversationExpansion.kept(
            both, selected: [SelectionKey(second.messages[0])], rows: rows
        )
        #expect(kept == [ConversationExpansion.key(second)])
    }
}
