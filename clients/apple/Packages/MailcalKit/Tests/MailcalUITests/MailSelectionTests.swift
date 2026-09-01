// The message list's selection rules (`docs/list-selection.md`), without a window: what each
// modifier means, what survives a snapshot rebuild, which of the paired actions the bar offers,
// and what a conversation row becomes when the batch goes to the core.
//
// The last two fail silently in the running app: a button offering the action that changes nothing
// does nothing when clicked, and a conversation sent as its latest message archives one reply
// while the rest of the thread stays in the inbox.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct MailSelectionTests {
    private func avatar() -> Avatar {
        Avatar(
            initials: "S",
            light: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
            dark: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
            imagePath: nil
        )
    }

    private func flat(_ key: String, unread: Bool = false, flagged: Bool = false) -> SnapshotRow {
        .flat(
            row: FlatRow(
                account: "acct-1",
                key: key,
                subject: "Subject \(key)",
                from: "sender",
                avatar: avatar(),
                date: "2026-07-20",
                unread: unread,
                flagged: flagged,
                hasAttachment: false,
                preview: ""
            )
        )
    }

    private func thread(_ id: String, unreadCount: UInt32 = 0) -> SnapshotRow {
        .thread(
            row: ThreadRow(
                account: "acct-1",
                threadId: id,
                latestKey: "\(id)-latest",
                subject: "Conversation",
                latestFrom: "sender",
                avatar: avatar(),
                latestDate: "2026-07-20",
                messageCount: 3,
                unreadCount: unreadCount,
                hasAttachment: false,
                preview: "",
                messages: []
            )
        )
    }

    private func keys(_ selection: MailSelection) -> [String] {
        selection.selectedRows.map { row in
            switch row {
            case .message(_, let key): return key
            case .thread(_, let threadId): return threadId
            }
        }
    }

    @Test func aPlainClickPicksOneRowAndDropsTheRest() {
        let rows = [flat("m1"), flat("m2")]
        var selection = MailSelection()

        selection.click(rows, 0, .toggle)
        selection.click(rows, 1, .toggle)
        #expect(keys(selection) == ["m1", "m2"])

        selection.click(rows, 1, .replace)
        #expect(keys(selection) == ["m2"], "a plain click starts over")
    }

    @Test func commandClickAddsThenRemovesTheSameRow() {
        let rows = [flat("m1"), flat("m2")]
        var selection = MailSelection()

        selection.click(rows, 0, .replace)
        selection.click(rows, 1, .toggle)
        #expect(keys(selection) == ["m1", "m2"])

        selection.click(rows, 1, .toggle)
        #expect(keys(selection) == ["m1"], "the second click deselects")
    }

    @Test func shiftClickTakesTheRangeBetweenTheAnchorAndTheRow() {
        let rows = [flat("m1"), flat("m2"), flat("m3"), flat("m4")]
        var selection = MailSelection()

        selection.click(rows, 1, .replace)
        selection.click(rows, 3, .range)
        #expect(keys(selection) == ["m2", "m3", "m4"])

        // Upwards from the same anchor, which is what correcting an over-long range does.
        selection.click(rows, 0, .range)
        #expect(keys(selection) == ["m1", "m2"])
    }

    @Test func aShiftClickWithNothingToExtendFromPicksTheOneRow() {
        var selection = MailSelection()
        selection.click([flat("m1"), flat("m2")], 1, .range)
        #expect(keys(selection) == ["m2"])
    }

    /// The archived rows are gone from the next snapshot. A selection that kept them would act on
    /// messages nobody can see, and the first sign of it would be mail leaving the mailbox.
    @Test func aRowThatLeavesTheListLeavesTheSelectionWithIt() {
        var selection = MailSelection()
        selection.selectAll([flat("m1"), flat("m2"), flat("m3")])

        selection.retainListed([flat("m2")])
        #expect(keys(selection) == ["m2"])

        selection.retainListed([])
        #expect(selection.isEmpty, "an emptied list empties the selection")
    }

    @Test func aRangeAfterItsAnchorLeftTheListPicksTheOneRow() {
        var selection = MailSelection()
        selection.click([flat("m1"), flat("m2")], 0, .replace)

        // m1 was archived; the anchor goes with it rather than pointing at whatever moved up.
        let rows = [flat("m2"), flat("m3")]
        selection.retainListed(rows)
        selection.click(rows, 1, .range)
        #expect(keys(selection) == ["m3"])
    }

    @Test func theBarOffersMarkReadWhileAnythingSelectedIsUnread() {
        let unread = [flat("m1", unread: true), flat("m2")]
        var selection = MailSelection()
        selection.selectAll(unread)
        #expect(selection.readAction(in: unread) == .markRead)

        let read = [flat("m1"), flat("m2")]
        #expect(selection.readAction(in: read) == .markUnread)
    }

    @Test func theBarOffersFlagWhileAnythingSelectedIsUnflagged() {
        let mixed = [flat("m1", flagged: true), flat("m2")]
        var selection = MailSelection()
        selection.selectAll(mixed)
        #expect(selection.flagAction(in: mixed) == .flag)

        let flagged = [flat("m1", flagged: true), flat("m2", flagged: true)]
        #expect(selection.flagAction(in: flagged) == .unflag)
    }

    /// The core expands a conversation itself, from the store's thread index; naming its latest
    /// message here would archive one reply and leave the rest of the thread in the inbox.
    @Test func aConversationIsSelectedAsAThreadNotAsItsLatestMessage() {
        let rows = [thread("t1", unreadCount: 2)]
        var selection = MailSelection()
        selection.selectAll(rows)

        #expect(selection.selectedRows.count == 1)
        if case .thread(let account, let threadId) = selection.selectedRows[0] {
            #expect(account == "acct-1")
            #expect(threadId == "t1")
        } else {
            Issue.record("a conversation row must travel as a thread")
        }
        #expect(selection.readAction(in: rows) == .markRead)
        #expect(
            selection.flagAction(in: rows) == .flag,
            "a conversation carries no flag of its own, so flagging is what it can be asked for"
        )
    }

    @Test func onlyTheActionsThatEmptyTheRowClearTheSelection() {
        #expect(BulkAction.archive.removesRows)
        #expect(BulkAction.delete.removesRows)
        #expect(BulkAction.permanentlyDelete.removesRows)
        #expect(!BulkAction.markRead.removesRows)
        #expect(!BulkAction.markUnread.removesRows)
        #expect(!BulkAction.flag.removesRows)
        #expect(!BulkAction.unflag.removesRows)
    }
}
