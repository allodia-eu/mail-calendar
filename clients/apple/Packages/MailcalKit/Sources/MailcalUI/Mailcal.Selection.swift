// The shell's half of the message list's multi-selection: what a click means on each platform,
// the bar over the list, and the batched action every one of its buttons runs.
//
// The rules are in `MailSelection`; this file is the wiring. Split from Mailcal.swift so the app
// shell stays under the repo line limit.

#if os(macOS)
import AppKit
#endif
import MailcalBindings
import SwiftUI

extension ContentView {
    /// What a click on a list row means, or `nil` when it is not about the selection at all.
    ///
    /// On macOS the modifiers decide, which is the Outlook behaviour the contract asks for. On
    /// iPhone and iPad there are none, so selecting is a mode: outside it a tap opens a message as
    /// it always did, and inside it a tap adds or removes a row.
    func selectionMode() -> SelectMode? {
        #if os(macOS)
        let flags = NSEvent.modifierFlags
        if flags.contains(.shift) { return .range }
        if flags.contains(.command) { return .toggle }
        return .replace
        #else
        return selectingRows ? .toggle : nil
        #endif
    }

    /// Handles a click on a list row: applies it to the selection and answers whether the row
    /// should also open. A modified click (or a tap in selection mode) is aimed at the selection
    /// alone; opening as well would fetch and display a body for every row added to a set.
    func applySelectionClick(_ row: SnapshotRow) -> Bool {
        guard let index = visibleRows.firstIndex(where: { $0.rowID == row.rowID }),
            let mode = selectionMode()
        else {
            return true
        }
        selection.click(visibleRows, index, mode)
        #if os(macOS)
        return mode == .replace
        #else
        return false
        #endif
    }

    /// Runs one action over the selection, then tidies up: a move empties the selection (its rows
    /// are leaving) and lets the reading pane go if the open message was in the batch; a keyword
    /// edit leaves both alone, because the user is usually part-way through a set.
    func actOnSelection(_ action: BulkAction) {
        guard !selection.isEmpty else { return }
        let closesReading = action.removesRows && selectionHoldsOpenMessage
        model.actOnSelection(selection.selectedRows, action)
        guard action.removesRows else { return }
        selection.clear()
        if closesReading { clearOpenedMessage() }
    }

    /// Runs a row menu's action over the **selection** when the menu's own row is part of it, and
    /// answers whether it did (`docs/list-selection.md`, rule 12). A menu opened on a row outside
    /// the selection is about that row alone, so this answers `false` and the caller takes its
    /// ordinary single-row path.
    ///
    /// The action is the one the user's menu item named, not the one rule 5 derives for the bar:
    /// they read a label and chose it, so "Mark as read" over a selection marks it read whatever
    /// mix of read and unread it holds.
    func actFromRowMenu(_ row: SnapshotRow, _ action: BulkAction) -> Bool {
        guard selection.contains(row) else { return false }
        actOnSelection(action)
        return true
    }

    /// Whether the message in the reading pane is one of the selected rows, a conversation's
    /// members included. The pane is cleared rather than advanced: the row it would advance to may
    /// be in the same batch and about to leave too.
    private var selectionHoldsOpenMessage: Bool {
        guard let opened = openedMessage else { return false }
        return visibleRows.filter(selection.contains).contains { row in
            switch row {
            case .flat(let message):
                return message.account == opened.account && message.key == opened.key
            case .thread(let thread):
                return thread.messages.contains {
                    $0.account == opened.account && $0.key == opened.key
                }
            }
        }
    }

    /// Drops selected rows the list no longer holds. Run on every snapshot, so a selection can
    /// never outlive the list it was made in (`docs/list-selection.md`, rule 4).
    func pruneSelection() {
        selection.retainListed(visibleRows)
    }

    /// The list's own selection behaviour: the pruning above, and, on the desktop, the two keys
    /// the contract binds.
    ///
    /// The keys hang off the **list**, never the window: bound app-wide, Delete would reach a
    /// focused search field or composer, where it has to keep editing text
    /// (`docs/list-selection.md`, rule 9). ⌘A is not bound at all, for the same reason it is a
    /// button on the bar: it would take the same key away from the search field.
    func selectionBehaviour(_ list: some View) -> AnyView {
        let pruned = list.onChange(of: visibleRows.map(\.rowID)) { _, _ in pruneSelection() }
        #if os(macOS)
        return AnyView(
            pruned
                .focusable()
                .focusEffectDisabled()
                .onKeyPress(.delete) { deleteSelectionByKey() }
                .onKeyPress(.deleteForward) { deleteSelectionByKey() }
                .onKeyPress(.escape) { clearSelectionByKey() }
        )
        #else
        return AnyView(pruned)
        #endif
    }

    #if os(macOS)
    /// Delete (and ⌦) moves the selection to Trash. Recoverable, so it asks nothing; an empty
    /// selection passes the key on rather than swallowing it.
    private func deleteSelectionByKey() -> KeyPress.Result {
        guard !selection.isEmpty else { return .ignored }
        actOnSelection(.delete)
        return .handled
    }

    private func clearSelectionByKey() -> KeyPress.Result {
        guard !selection.isEmpty else { return .ignored }
        selection.clear()
        return .handled
    }
    #endif

    /// The bar of message actions: the count, and the actions the contract gives it.
    ///
    /// On macOS it is **standing chrome** over both the list and the reading pane, and its actions
    /// are merely disabled while nothing is picked (`docs/list-selection.md`, rule 5): a bar that
    /// arrives with the first click moves the rows under the pointer and the message being read,
    /// which is the jump Outlook's own toolbar does not have. On the iPhone and the iPad selecting
    /// is a mode, so the bar is that mode's chrome and comes and goes with it.
    @ViewBuilder
    var selectionBar: some View {
        #if os(macOS)
        selectionBarRow
        #else
        if !selection.isEmpty { selectionBarRow }
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
        let read = selection.readAction(in: visibleRows)
        let flag = selection.flagAction(in: visibleRows)
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
        Button { endSelecting() } label: {
            Label(L10n.action_clear_selection(), systemImage: "xmark")
        }
        .disabled(selection.isEmpty)
    }

    /// Leaves the selection, and on the phone and iPad the mode with it.
    func endSelecting() {
        selection.clear()
        #if os(iOS)
        selectingRows = false
        #endif
    }

    #if os(iOS)
    /// The toolbar's way into selection mode, and back out of it. iPhone and iPad have no
    /// modifiers, so the mode is the whole affordance.
    @ViewBuilder
    var selectToggleButton: some View {
        Button(selectingRows ? L10n.action_done() : L10n.action_select()) {
            if selectingRows {
                endSelecting()
            } else {
                selectingRows = true
            }
        }
    }
    #endif
}
