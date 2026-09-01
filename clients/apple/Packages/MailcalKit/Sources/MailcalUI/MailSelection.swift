// The message list's multi-selection: which rows are picked, and what the bar over them offers.
//
// Pure value state over the snapshot's row order, so the rules (`docs/list-selection.md`) are
// testable without a window. The views read it; they keep no second copy.

import Foundation
import MailcalBindings

/// One selected row's identity: the account, plus a message key or a thread id.
///
/// Account-scoped, because a provider key is unique only within its account and the unified list
/// shows several at once. `isThread` decides which shape the row travels to the core in: a
/// conversation must go as a thread, since the core expands it from the store's thread index,
/// which holds messages the list never showed.
struct SelectionKey: Hashable {
    let account: String
    let id: String
    let isThread: Bool

    init(_ row: SnapshotRow) {
        switch row {
        case .flat(let message):
            account = message.account
            id = message.key
            isThread = false
        case .thread(let thread):
            account = thread.account
            id = thread.threadId
            isThread = true
        }
    }

    var selectedRow: SelectedRow {
        isThread
            ? .thread(account: account, threadId: id)
            : .message(account: account, key: id)
    }
}

/// What a click on a row means, decided by the modifiers the user held (macOS) or by the mode the
/// list is in (iPhone and iPad, which have no modifiers).
enum SelectMode {
    /// A plain click: this row alone, and it becomes the anchor.
    case replace
    /// ⌘-click, or a tap in selection mode: add or remove this row; it becomes the anchor.
    case toggle
    /// ⇧-click: every row from the anchor to this one, replacing what was selected.
    case range
}

/// The rows the user has picked, in list order, plus the anchor a range extends from.
struct MailSelection {
    private(set) var keys: [SelectionKey] = []
    private var anchor: SelectionKey?

    var isEmpty: Bool { keys.isEmpty }
    var count: Int { keys.count }

    func contains(_ row: SnapshotRow) -> Bool { keys.contains(SelectionKey(row)) }

    /// Applies a click on the row at `index` of `rows`.
    mutating func click(_ rows: [SnapshotRow], _ index: Int, _ mode: SelectMode) {
        guard rows.indices.contains(index) else { return }
        let clicked = SelectionKey(rows[index])
        switch mode {
        case .replace:
            keys = [clicked]
            anchor = clicked
        case .toggle:
            if let position = keys.firstIndex(of: clicked) {
                keys.remove(at: position)
            } else {
                keys.append(clicked)
            }
            anchor = clicked
        case .range:
            // Nothing to extend from (the first click of the session was a ⇧-click) means there is
            // no range either; take the one row, which is what every list with no anchor does.
            guard let anchor, let start = rows.firstIndex(where: { SelectionKey($0) == anchor })
            else {
                keys = [clicked]
                self.anchor = clicked
                return
            }
            let range = min(start, index)...max(start, index)
            keys = rows[range].map(SelectionKey.init)
        }
    }

    /// Selects every row the list is showing, which is the loaded window rather than the whole
    /// folder (`docs/list-selection.md`, rule 10).
    mutating func selectAll(_ rows: [SnapshotRow]) {
        keys = rows.map(SelectionKey.init)
        anchor = keys.first
    }

    mutating func clear() {
        keys = []
        anchor = nil
    }

    /// Drops anything `rows` no longer holds: a message that was archived, a folder the user has
    /// left, a search that replaced the list. A selection outliving its list acts on rows nobody
    /// can see (`docs/list-selection.md`, rule 4).
    mutating func retainListed(_ rows: [SnapshotRow]) {
        let listed = Set(rows.map(SelectionKey.init))
        keys = keys.filter(listed.contains)
        if let anchor, !listed.contains(anchor) { self.anchor = nil }
    }

    /// The selected rows in the shape `Intent.actOnSelection` takes.
    var selectedRows: [SelectedRow] { keys.map(\.selectedRow) }

    /// The action the bar's single read button runs: the one that changes something, so any unread
    /// row makes it "mark read" (`docs/list-selection.md`, rule 5).
    func readAction(in rows: [SnapshotRow]) -> BulkAction {
        selected(in: rows).contains(where: isUnread) ? .markRead : .markUnread
    }

    /// The action the bar's single flag button runs, on the same terms. A conversation carries no
    /// flag of its own, so it counts as unflagged: flagging is what its rows can be asked for.
    func flagAction(in rows: [SnapshotRow]) -> BulkAction {
        selected(in: rows).contains(where: isUnflagged) ? .flag : .unflag
    }

    private func selected(in rows: [SnapshotRow]) -> [SnapshotRow] { rows.filter(contains) }

    private func isUnread(_ row: SnapshotRow) -> Bool {
        switch row {
        case .flat(let message): return message.unread
        case .thread(let thread): return thread.unreadCount > 0
        }
    }

    private func isUnflagged(_ row: SnapshotRow) -> Bool {
        switch row {
        case .flat(let message): return !message.flagged
        case .thread: return true
        }
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
