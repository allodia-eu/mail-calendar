// The message list's selection rules (`docs/list-selection.md`), without a window: what survives a
// snapshot rebuild, which of the paired actions the bar offers, and what a conversation row and a
// message picked inside one become when the batch goes to the core.
//
// What a click or a modifier does is the list control's and is not restated here. The rest fail
// silently in the running app: a button offering the action that changes nothing does nothing when
// clicked, and a conversation sent as its latest message archives one reply while the rest of the
// thread stays in the inbox.

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

    private func member(_ key: String, unread: Bool = false) -> ThreadMessage {
        ThreadMessage(
            account: "acct-1", key: key, from: "sender", avatar: avatar(), date: "2026-07-20",
            preview: "", unread: unread, outgoing: false, draft: false, hasAttachment: false
        )
    }

    private func conversation(
        _ id: String, unreadCount: UInt32 = 0, messages: [ThreadMessage] = []
    ) -> ThreadRow {
        ThreadRow(
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
                messages: messages
        )
    }

    private func thread(_ id: String, unreadCount: UInt32 = 0) -> SnapshotRow {
        .thread(row: conversation(id, unreadCount: unreadCount))
    }

    private func keys(_ selection: MailSelection, expanded: [ThreadRow] = []) -> [String] {
        selection.selectedRows(expanded: expanded).map { row in
            switch row {
            case .message(_, let key): return key
            case .thread(_, let threadId): return threadId
            }
        }
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

    /// Folding a conversation away takes its messages off the list, so a message picked inside
    /// it leaves the selection with them.
    @Test func aMessageOfAFoldedConversationLeavesTheSelection() {
        let open = conversation("t1", messages: [member("t1-a"), member("t1-b")])
        var selection = MailSelection()
        selection.keys = [SelectionKey(open.messages[0])]

        selection.retainListed([.thread(row: open)], expanded: [open])
        #expect(keys(selection) == ["t1-a"], "still listed while the conversation is open")

        selection.retainListed([.thread(row: open)])
        #expect(selection.isEmpty)
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

    /// The desktop bar stands whether or not anything is picked, so these two labels are read
    /// with an empty selection. "Mark as unread" over nothing is the wrong sentence to leave on
    /// screen: the affirmative action is the one the bar names until a selection says otherwise.
    @Test func anEmptySelectionNamesTheAffirmativeActions() {
        let rows = [flat("m1"), flat("m2", flagged: true)]
        let selection = MailSelection()
        #expect(selection.readAction(in: rows) == .markRead)
        #expect(selection.flagAction(in: rows) == .flag)
    }

    /// A plain click selects a row *and* opens it, so a pane that stated "1 selected" would
    /// replace the message the user just asked to read with a count of it.
    @Test func theReadingPaneStatesTheCountOnlyAboveOneRow() {
        let rows = [flat("m1"), flat("m2"), flat("m3")]
        var selection = MailSelection()
        #expect(selection.paneLabel(mode: .flat) == nil, "nothing picked, nothing to say")

        selection.keys = [SelectionKey(rows[0])]
        #expect(selection.paneLabel(mode: .flat) == nil, "the one row is what the pane is showing")

        selection.selectAll(rows)
        // The words are the catalog's; what this asserts is that the list the rows were picked in
        // decides which noun, since a threaded row stands for a whole conversation.
        let flatLabel = selection.paneLabel(mode: .flat)
        let threadedLabel = selection.paneLabel(mode: .threaded)
        #expect(flatLabel != nil)
        #expect(flatLabel != threadedLabel)
    }

    /// The core expands a conversation itself, from the store's thread index; naming its latest
    /// message here would archive one reply and leave the rest of the thread in the inbox.
    @Test func aConversationIsSelectedAsAThreadNotAsItsLatestMessage() {
        let rows = [thread("t1", unreadCount: 2)]
        var selection = MailSelection()
        selection.selectAll(rows)

        #expect(selection.selectedRows().count == 1)
        if case .thread(let account, let threadId) = selection.selectedRows()[0] {
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

    @Test func aSelectedConversationIsNotTheMessageRowThatSharesItsId() {
        var selection = MailSelection()
        selection.selectAll([thread("x")])

        // The two travel to the core as different shapes, so a message menu may not act on a
        // selection holding only the conversation whose id happens to match.
        #expect(!selection.contains(flat("x")))
    }

    /// A message picked inside a conversation travels as the message it is, and is left out when
    /// its whole conversation is picked too: the core expands the conversation to it already.
    @Test func aMessageInsideASelectedConversationIsNotSentTwice() {
        let open = conversation("t1", messages: [member("t1-a"), member("t1-b")])
        var selection = MailSelection()
        selection.keys = [SelectionKey(open.messages[1])]
        #expect(keys(selection, expanded: [open]) == ["t1-b"])
        if case .message = selection.selectedRows(expanded: [open])[0] {} else {
            Issue.record("a message picked inside a conversation must travel as a message")
        }

        selection.keys.insert(SelectionKey(.thread(row: open)))
        #expect(keys(selection, expanded: [open]) == ["t1"])
    }

    @Test func aMessagePickedInsideAConversationCountsForTheBar() {
        let open = conversation("t1", messages: [member("t1-a", unread: true)])
        var selection = MailSelection()
        selection.keys = [SelectionKey(open.messages[0])]

        #expect(selection.readAction(in: [.thread(row: open)], expanded: [open]) == .markRead)
        #expect(selection.flagAction(in: [.thread(row: open)], expanded: [open]) == .flag)
    }

    /// A threaded list calls its rows conversations, a single-message one drawn as a message row
    /// included, but a message picked inside an expanded conversation is a message.
    @Test func thePaneCallsAMessagePickedInsideAConversationAMessage() {
        let open = conversation("t1", messages: [member("t1-a")])
        var selection = MailSelection()
        selection.keys = [SelectionKey(.thread(row: open)), SelectionKey(flat("m1"))]
        let conversations = selection.paneLabel(mode: .threaded, expanded: [open])
        #expect(conversations != selection.paneLabel(mode: .flat))

        selection.keys = [SelectionKey(.thread(row: open)), SelectionKey(open.messages[0])]
        #expect(selection.paneLabel(mode: .threaded, expanded: [open]) != conversations)
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
