// The message list every Apple shell draws: one `List` whose selection is the toolkit's.
//
// Clicks, modifiers, a drag across rows, the arrow keys, ⌘A and Edit ▸ Select All are the list
// control's, so each means on every Apple platform what it means in every other app there
// (`docs/list-selection.md`, rule 1). What the app adds is what a selection *does*: opening the one
// row picked, the menus over it, and the batch the bar runs.

#if os(macOS)
import AppKit
#endif
import MailcalBindings
import SwiftUI

/// One row of the message list: a row of the snapshot, or a message of an expanded conversation,
/// which is a row of its own so it can be selected and opened as the message it is.
enum MessageListEntry: Identifiable {
    case row(SnapshotRow)
    case member(ThreadRow, ThreadMessage)

    var id: String {
        switch self {
        case .row(let row): return row.rowID
        case .member(let thread, let message):
            return "\(SnapshotRow.thread(row: thread).rowID):\(message.key)"
        }
    }

    var key: SelectionKey {
        switch self {
        case .row(let row): return SelectionKey(row)
        case .member(_, let message): return SelectionKey(message)
        }
    }
}

#if os(macOS)
/// The pane and the selection as they were before a click opened a row, and when.
struct PaneBeforeClick {
    let opened: OpenedMessage?
    let keys: Set<SelectionKey>
    let at: Date
}
#endif

extension ContentView {
    /// The conversations on screen whose messages are rows of their own.
    var expandedConversations: [ThreadRow] {
        visibleRows.compactMap { row in
            guard case .thread(let thread) = row, expandedThreads.contains(threadKey(thread)) else {
                return nil
            }
            return thread
        }
    }

    /// The list's rows in order: each snapshot row, followed by its messages where it is an
    /// expanded conversation.
    var listEntries: [MessageListEntry] {
        visibleRows.flatMap { row -> [MessageListEntry] in
            guard case .thread(let thread) = row, expandedThreads.contains(threadKey(thread)) else {
                return [.row(row)]
            }
            return [.row(row)] + thread.messages.map { .member(thread, $0) }
        }
    }

    /// The list, with the selection behaviour every shell shares. Each shell adds its own style
    /// and the surfaces around it.
    var messageRows: some View {
        let rows = visibleRows
        let list = List(selection: $selection.keys) {
            ForEach(listEntries) { entry in
                entryView(entry)
                    .tag(entry.key)
                    .listRowBackground(readingHighlight(entry))
                    // Infinite scroll: when the last row appears, ask the core for the next page.
                    // Deferred to the next main-actor hop so the list is not grown from inside the
                    // table's layout pass (the NSTableView "reentrant operation" warning).
                    .onAppear {
                        if case .row(let row) = entry, row.rowID == rows.last?.rowID {
                            Task { @MainActor in model.showMore() }
                        }
                    }
            }
        }
        // One menu for the list rather than one per row, because the toolkit then decides what it
        // is about: the selection when the row under the pointer is in it, that row alone when it
        // is not, and the selection is left as it was (`docs/list-selection.md`, rule 12).
        .contextMenu(forSelectionType: SelectionKey.self) { keys in
            rowMenu(keys)
        } primaryAction: { keys in
            primaryAction(keys)
        }
        .onChange(of: selection.keys) { old, keys in
            // Leaving a conversation folds it away (`docs/row-actions.md`, rule 8).
            expandedThreads = ConversationExpansion.kept(
                expandedThreads, selected: keys, rows: visibleRows
            )
            let before = openedMessage
            let opened = openSoleSelection(keys)
            #if os(macOS)
            if opened { paneBeforeClick = PaneBeforeClick(opened: before, keys: old, at: .now) }
            // A SwiftUI list on the Mac selects a clicked row without taking focus, which left
            // ⌘A, Edit ▸ Select All and Delete with the sidebar, so a row the user picked hands
            // the list focus. Only a selection that gained a row: one that shrank because rows
            // left the list must not pull focus out of the search field being typed in.
            if !keys.isSubset(of: old) { messageListFocused = true }
            #endif
        }
        .onChange(of: listEntries.map(\.id)) { _, _ in pruneSelection() }
        .onChange(of: openedMessage) { _, opened in followOpenedMessage(opened) }
        .onChange(of: compose == nil) { _, closed in
            // A draft opens a composer rather than the pane, so nothing else lets go of the
            // row when the composer closes, and picking the same row again would change nothing.
            if closed, openedMessage == nil, selection.count == 1 { selection.clear() }
        }
        #if os(macOS)
        // Edit ▸ Delete and the Delete key, and Escape, while the list has focus. Bound on the
        // list, never the window, so a focused field keeps its own keys (rule 9).
        return list
            .focused($messageListFocused)
            .onDeleteCommand { actOnSelection(.delete) }
            .onExitCommand { selection.clear() }
        #else
        return list.environment(\.editMode, $editMode)
        #endif
    }

    /// Opens the row the user picked, when it is the only one, and answers whether the pane took
    /// it. A plain click selects a row and opens it; a second row joining the selection is about
    /// the set, so it opens nothing.
    private func openSoleSelection(_ keys: Set<SelectionKey>) -> Bool {
        #if os(iOS)
        // In selection mode a tap adds a row to the set, it does not read it.
        guard !editMode.isEditing else { return false }
        #endif
        guard keys.count == 1, let key = keys.first else { return false }
        return openEntry(key)
    }

    /// Opens what `key` names, unless the pane already shows it, and answers whether the pane took
    /// it. A draft opens a composer instead, which nothing can put back, so it answers no.
    @discardableResult
    private func openEntry(_ key: SelectionKey) -> Bool {
        switch listEntries.first(where: { $0.key == key }) {
        case .row(.flat(let message)):
            guard !isOpenMessage(message.account, message.key) else { return false }
            open(message)
            return !message.draft
        case .row(.thread(let thread)):
            guard !threadHoldsOpenMessage(thread) else {
                // Already the one being read, folded by its chevron: picking it opens it again.
                expandedThreads.insert(threadKey(thread))
                return false
            }
            open(thread)
            return representative(of: thread)?.draft == false
        case .member(let thread, let message):
            guard !isOpenMessage(message.account, message.key) else { return false }
            openThreadMessage(thread, message)
            return !message.draft
        case nil:
            return false
        }
    }

    /// Keeps the selection on the message the pane shows when something other than a click
    /// changed it: the pane advancing after an archive, a notification, a restored scene. A set
    /// of several is the user's and is left alone, and so is a conversation row standing for the
    /// message it holds.
    private func followOpenedMessage(_ opened: OpenedMessage?) {
        #if os(iOS)
        guard !editMode.isEditing else { return }
        #endif
        guard selection.count <= 1 else { return }
        guard let opened else {
            // Nothing open, so nothing selected: on the iPhone that is Back from the message,
            // after which tapping the same row has to open it again.
            selection.clear()
            return
        }
        if let current = selection.keys.first, entry(current, shows: opened) { return }
        if let entry = listEntries.first(where: { self.entry($0.key, holdsExactly: opened) }) {
            selection.keys = [entry.key]
        }
    }

    /// Whether the row `key` names is the open message, or a conversation holding it.
    private func entry(_ key: SelectionKey, shows opened: OpenedMessage) -> Bool {
        if entry(key, holdsExactly: opened) { return true }
        return expandedConversations.contains { thread in
            SelectionKey(.thread(row: thread)) == key && threadHoldsOpenMessage(thread)
        }
    }

    /// Whether `key` names the open message itself, or a folded conversation that holds it.
    private func entry(_ key: SelectionKey, holdsExactly opened: OpenedMessage) -> Bool {
        if !key.isThread { return key.account == opened.account && key.id == opened.key }
        return visibleRows.contains { row in
            guard case .thread(let thread) = row, SelectionKey(row) == key,
                !expandedThreads.contains(threadKey(thread))
            else { return false }
            return threadHoldsOpenMessage(thread)
        }
    }

    func threadHoldsOpenMessage(_ thread: ThreadRow) -> Bool {
        thread.messages.contains { isOpenMessage($0.account, $0.key) }
    }

    /// The open message's row, drawn lightly where it is not also selected: a conversation folded
    /// over it, or a row the pane kept while the selection moved on. A selected row keeps the
    /// toolkit's own highlight.
    private func readingHighlight(_ entry: MessageListEntry) -> Color? {
        guard !selection.keys.contains(entry.key) else { return nil }
        let reading: Bool
        switch entry {
        case .row(.flat(let message)): reading = isOpenMessage(message.account, message.key)
        case .row(.thread(let thread)):
            reading = !expandedThreads.contains(threadKey(thread)) && threadHoldsOpenMessage(thread)
        case .member(_, let message): reading = isOpenMessage(message.account, message.key)
        }
        return reading ? Color.accentColor.opacity(0.15) : nil
    }

    /// Double-click or Return on the Mac, a tap outside selection mode on iPhone and iPad.
    private func primaryAction(_ keys: Set<SelectionKey>) {
        guard keys.count == 1, let key = keys.first else { return }
        switch listEntries.first(where: { $0.key == key }) {
        case .row(.flat(let message)):
            #if os(macOS)
            // A window of its own; the pane and the selection stay as they were
            // (`docs/reading-window.md`).
            putPaneBack()
            openInWindow(message)
            #else
            open(message)
            #endif
        case .row(.thread(let thread)):
            toggle(thread)
        case .member(let thread, let message):
            #if os(macOS)
            putPaneBack()
            openInWindow(thread, message)
            #else
            openThreadMessage(thread, message)
            #endif
        case nil:
            break
        }
    }

    #if os(macOS)
    /// Undoes what the first press of a double-click did. The list selects and opens a row on that
    /// press, before anything can know a window was wanted, so the pane and the selection go back
    /// to what they held. A primary action with no click just before it (Return) has nothing to
    /// undo.
    private func putPaneBack() {
        defer { paneBeforeClick = nil }
        guard let before = paneBeforeClick,
            Date.now.timeIntervalSince(before.at) <= NSEvent.doubleClickInterval
        else { return }
        if let opened = before.opened { openNow(opened) } else { clearOpenedMessage() }
        selection.keys = before.keys
    }
    #endif
}
