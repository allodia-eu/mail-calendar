// The shell's half of the message list's multi-selection: the batched action the bar and the row
// menus run, and keeping the selection to what the list shows. The bar is Mailcal.CommandBar.swift.
//
// The rules are in `MailSelection`, the list's own behaviour in Mailcal.MessageList.swift; this
// file is the wiring. Split from Mailcal.swift so the app shell stays under the repo line limit.

import MailcalBindings
import SwiftUI

extension ContentView {
    /// Runs one action over the selection, then tidies up: a move empties the selection (its rows
    /// are leaving) and lets the reading pane go if the open message was in the batch; a keyword
    /// edit leaves both alone, because the user is usually part-way through a set.
    func actOnSelection(_ action: BulkAction) {
        guard !selection.isEmpty else { return }
        let closesReading = action.removesRows && holdsOpenMessage(selection.keys)
        model.actOnSelection(selection.selectedRows(expanded: expandedConversations), action)
        guard action.removesRows else { return }
        selection.clear()
        if closesReading { clearOpenedMessage() }
    }

    /// Whether the message in the reading pane is one of `keys`' rows, a conversation's members
    /// included. The pane is cleared rather than advanced: the row it would advance to may be in
    /// the same batch and about to leave too.
    func holdsOpenMessage(_ keys: some Collection<SelectionKey>) -> Bool {
        guard let opened = openedMessage else { return false }
        let isOpen = { (account: String, key: String) in
            account == opened.account && key == opened.key
        }
        if keys.contains(where: { !$0.isThread && isOpen($0.account, $0.id) }) { return true }
        return visibleRows.contains { row in
            guard case .thread(let thread) = row, keys.contains(SelectionKey(row)) else {
                return false
            }
            return thread.messages.contains { isOpen($0.account, $0.key) }
        }
    }

    /// Drops selected rows the list no longer holds. Run whenever the rows change, so a selection
    /// can never outlive the list it was made in (`docs/list-selection.md`, rule 4).
    func pruneSelection() {
        selection.retainListed(visibleRows, expanded: expandedConversations)
    }

    /// Leaves the selection, and on the phone and iPad the mode with it.
    func endSelecting() {
        selection.clear()
        #if os(iOS)
        withAnimation { editMode = .inactive }
        #endif
    }

    #if os(iOS)
    /// The toolbar's way into the list's edit mode, and back out of it. On a touch screen a row is
    /// selected only in that mode, which draws the system's own checkmarks.
    @ViewBuilder
    var selectToggleButton: some View {
        Button(editMode.isEditing ? L10n.action_done() : L10n.action_select()) {
            if editMode.isEditing {
                endSelecting()
            } else {
                selection.clear()
                withAnimation { editMode = .active }
            }
        }
    }
    #endif
}
