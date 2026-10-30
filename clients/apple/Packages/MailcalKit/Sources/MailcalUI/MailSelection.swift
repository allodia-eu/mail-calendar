// The message list's multi-selection: which rows are picked, and what they become when the batch
// goes to the core. What the bar over them offers is the core's (`RowMenuPresentation.swift`).
//
// Pure value state over the snapshot's rows, so the rules (`docs/list-selection.md`) are
// testable without a window. The views read it; they keep no second copy.

import Foundation
import MailcalBindings

/// One selectable row's identity: the account, plus a message key or a thread id. It is also the
/// tag each row carries in the list, so the toolkit's selection is a set of these.
///
/// Account-scoped, because a provider key is unique only within its account and the unified list
/// shows several at once. `isThread` decides which shape the row travels to the core in: a
/// conversation must go as a thread, since the core expands it from the store's thread index,
/// which holds messages the list never showed. A message of an expanded conversation is a row of
/// its own and travels as the message it is.
struct SelectionKey: Hashable {
    let account: String
    let id: String
    let isThread: Bool

    init(_ row: SnapshotRow) {
        switch row {
        case .flat(let message):
            self.init(account: message.account, id: message.key, isThread: false)
        case .thread(let thread):
            self.init(account: thread.account, id: thread.threadId, isThread: true)
        }
    }

    /// A message of an expanded conversation, picked out on its own.
    init(_ message: ThreadMessage) {
        self.init(account: message.account, id: message.key, isThread: false)
    }

    private init(account: String, id: String, isThread: Bool) {
        self.account = account
        self.id = id
        self.isThread = isThread
    }

    var selectedRow: SelectedRow {
        isThread
            ? .thread(account: account, threadId: id)
            : .message(account: account, key: id)
    }
}

/// The rows the user has picked. The list control owns how a click, a modifier, a drag or ⌘A
/// changes it (`docs/list-selection.md`, rule 1); this holds what the bar and the menus derive
/// from it.
struct MailSelection {
    var keys: Set<SelectionKey> = []

    var isEmpty: Bool { keys.isEmpty }
    var count: Int { keys.count }

    func contains(_ row: SnapshotRow) -> Bool { keys.contains(SelectionKey(row)) }

    /// Selects every row the list is showing, which is the loaded window rather than the whole
    /// folder (`docs/list-selection.md`, rule 10). The bar's Select all; ⌘A is the toolkit's.
    mutating func selectAll(_ rows: [SnapshotRow]) {
        keys = Set(rows.map(SelectionKey.init))
    }

    mutating func clear() {
        keys = []
    }

    /// Drops anything `rows` no longer holds: a message that was archived, a folder the user has
    /// left, a search that replaced the list, a conversation folded away. A selection outliving
    /// its list acts on rows nobody can see (`docs/list-selection.md`, rule 4).
    ///
    /// `expanded` is the conversations whose messages are rows of their own.
    mutating func retainListed(_ rows: [SnapshotRow], expanded: [ThreadRow] = []) {
        let listed = Set(rows.map(SelectionKey.init))
            .union(expanded.flatMap(\.messages).map(SelectionKey.init))
        keys = keys.intersection(listed)
    }

    /// The selected rows in the shape `Intent.actOnSelection` takes, in a stable order.
    ///
    /// A message picked inside a conversation that is itself selected is left out: the core
    /// expands the conversation to every message on it already, and naming one of them twice
    /// would write it twice. `expanded` is the conversations whose messages are rows.
    func selectedRows(expanded: [ThreadRow] = []) -> [SelectedRow] {
        let covered = Set(
            expanded.filter { keys.contains(SelectionKey(.thread(row: $0))) }
                .flatMap(\.messages).map(SelectionKey.init)
        )
        return keys.subtracting(covered)
            .sorted { ($0.account, $0.id) < ($1.account, $1.id) }
            .map(\.selectedRow)
    }

    /// What the reading pane says instead of a message while several rows are picked, or `nil`
    /// when it should go on showing what it holds.
    ///
    /// **More than one**, never one: a single click both selects a row and opens it, so stating
    /// "1 selected" over the pane would replace the message the user just asked to read with a
    /// count of it. The rows are called what the list calls them: conversations in the threaded
    /// list, unless a message picked inside an expanded conversation is among them, since that
    /// one is a message.
    func paneLabel(mode: ViewMode, expanded: [ThreadRow] = []) -> String? {
        guard count > 1 else { return nil }
        if mode == .threaded, selected(in: expanded).isEmpty {
            return L10n.selection_selected_conversations(count: count)
        }
        return L10n.selection_selected_messages(count: count)
    }

    private func selected(in expanded: [ThreadRow]) -> [ThreadMessage] {
        expanded.flatMap(\.messages).filter { keys.contains(SelectionKey($0)) }
    }
}

extension BulkAction {
    /// Whether the action takes its rows out of the folder, so the selection has nothing left to
    /// name afterwards. Read and flag change a message in place and leave the set alone, because
    /// the user is usually part-way through working through it.
    var removesRows: Bool {
        switch self {
        case .archive, .delete, .permanentlyDelete: return true
        case .markRead, .markUnread, .flag, .unflag: return false
        }
    }
}
