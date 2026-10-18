// The shell's half of the message list's multi-selection: the bar over the list, and the batched
// action every one of its buttons runs.
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

    /// The bar of message actions: the actions the contract gives it, plus, on the desktop, Sync.
    ///
    /// On macOS it is **standing chrome** over both the list and the reading pane, and its actions
    /// are merely disabled while nothing is picked (`docs/list-selection.md`, rule 5): a bar that
    /// arrives with the first click moves the rows under the pointer and the message being read,
    /// which is the jump Outlook's own toolbar does not have. On the iPhone and the iPad selecting
    /// is the list's edit mode, so the bar is that mode's chrome and comes and goes with it.
    @ViewBuilder
    var selectionBar: some View {
        #if os(macOS)
        selectionBarRow
        #else
        if editMode.isEditing { selectionBarRow }
        #endif
    }

    private var selectionBarRow: some View {
        HStack(spacing: 18) {
            // Leading, where a toolbar's buttons live and where Outlook puts the same six: at the
            // trailing edge they moved with every drag of the divider under them.
            selectionButtons
            Spacer()
        }
        #if os(macOS)
        // Icon and word together, which is what the bar's full width across both panes buys: the
        // glyph is what the eye finds and the word is what settles which trash can it is.
        .labelStyle(.titleAndIcon)
        #else
        // Icons alone: six labelled buttons do not fit a list column the user can drag to 420
        // points, and a bar that overflows hides the delete they were reaching for. Each button
        // is still a `Label`, so its name is what assistive technology reads.
        .labelStyle(.iconOnly)
        #endif
        .buttonStyle(.borderless)
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(.quaternary.opacity(0.5))
    }

    @ViewBuilder
    private var selectionButtons: some View {
        // One button per pair, labelled from what is selected, so the offered action is always the
        // one that changes something.
        let read = selection.readAction(in: visibleRows, expanded: expandedConversations)
        let flag = selection.flagAction(in: visibleRows, expanded: expandedConversations)
        Group {
            Button {
                actOnSelection(read)
            } label: {
                Label(
                    read == .markRead ? L10n.action_mark_read() : L10n.action_mark_unread(),
                    systemImage: read == .markRead ? "envelope.open" : "envelope.badge"
                )
            }
            Button {
                actOnSelection(flag)
            } label: {
                Label(
                    flag == .flag ? L10n.action_flag() : L10n.action_unflag(),
                    systemImage: flag == .flag ? "flag" : "flag.slash"
                )
            }
            Button { actOnSelection(.archive) } label: {
                Label(L10n.action_archive(), systemImage: "archivebox")
            }
            Button { actOnSelection(.delete) } label: {
                Label(L10n.action_move_to_trash(), systemImage: "trash")
            }
            // The row's own menu already offers this on every Apple platform, so the bar does too.
            // It asks nothing extra here for the same reason it asks nothing there.
            Button(role: .destructive) { actOnSelection(.permanentlyDelete) } label: {
                Label(L10n.action_delete_permanently(), systemImage: "trash.slash")
            }
        }
        // Every action names the selection, so with none they have nothing to act on. Select all
        // is the exception below: it is how a pointer starts a selection in the first place.
        .disabled(selection.isEmpty)
        Button { selection.selectAll(visibleRows) } label: {
            Label(L10n.action_select_all(), systemImage: "checklist")
        }
        #if os(iOS)
        // ⌘A from a hardware keyboard, while the mode is on: SwiftUI's list answers the system's
        // Select All only on the Mac. Outside the mode a touch screen selects nothing, so there is
        // no ⌘A there either.
        .keyboardShortcut("a", modifiers: .command)
        #endif
        Button { endSelecting() } label: {
            Label(L10n.action_clear_selection(), systemImage: "xmark")
        }
        .disabled(selection.isEmpty)
        #if os(macOS)
        // Sync stands on the same bar and is **not** a selection action: it acts on the
        // mailbox, so it is never disabled, and it sits past a divider rather than among the
        // buttons that grey out together (`docs/list-selection.md`, rule 5). The phones get the
        // pull-to-refresh gesture instead, so their bar, which is the selection mode's own
        // chrome, keeps only the actions that mode is for.
        //
        // The divider carries a height because a `Divider` in an `HStack` takes every point it
        // is offered: left free it stretched the bar over the whole window and pushed the
        // message list halfway down it, with the buttons centred in the hole.
        Divider().frame(height: 20)
        Button { model.refresh() } label: {
            Label(L10n.action_refresh(), systemImage: "arrow.clockwise")
        }
        #endif
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
