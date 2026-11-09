// What the core's row menu and command bar look like on Apple, and the one rule about expanding a conversation
// (`docs/row-actions.md`). The core decides which items a menu holds, which respond and which
// message each is about; this file gives each action its words and its symbol, and keeps no rule
// of its own about either. Pure, so it is tested without a window.

import Foundation
import MailcalBindings

extension RowAction {
    /// The item's label, in the user's language.
    var title: String {
        switch self {
        case .open: return L10n.action_open()
        case .openInWindow: return L10n.action_open_in_window()
        case .reply: return L10n.action_reply()
        case .replyAll: return L10n.action_reply_all()
        case .forward: return L10n.action_forward()
        case .markRead: return L10n.action_mark_read()
        case .markUnread: return L10n.action_mark_unread()
        case .flag: return L10n.action_flag()
        case .unflag: return L10n.action_clear_flag()
        case .markAsSpam: return L10n.action_mark_as_spam()
        case .markAsNotSpam: return L10n.action_mark_as_not_spam()
        case .moveToFolder: return L10n.action_move_to_folder()
        case .archive: return L10n.action_archive()
        case .delete: return L10n.action_delete()
        case .permanentlyDelete: return L10n.action_delete_permanently()
        }
    }

    /// The item's SF Symbol (`docs/icons.md`).
    var systemImage: String {
        switch self {
        case .open: return "envelope.open"
        case .openInWindow: return "macwindow.on.rectangle"
        case .reply: return "arrowshape.turn.up.left"
        case .replyAll: return "arrowshape.turn.up.left.2"
        case .forward: return "arrowshape.turn.up.right"
        case .markRead: return "envelope.open"
        case .markUnread: return "envelope.badge"
        case .flag: return "flag"
        case .unflag: return "flag.slash"
        case .markAsSpam: return "xmark.bin"
        case .markAsNotSpam: return "tray.and.arrow.down"
        case .moveToFolder: return "folder"
        case .archive: return "archivebox"
        case .delete: return "trash"
        case .permanentlyDelete: return "trash.slash"
        }
    }

    /// The batch an item without a target runs over its rows, or `nil` for the actions that
    /// always name one message (open, answer, report) or ask for a folder first.
    var bulkAction: BulkAction? {
        switch self {
        case .markRead: return .markRead
        case .markUnread: return .markUnread
        case .flag: return .flag
        case .unflag: return .unflag
        case .archive: return .archive
        case .delete: return .delete
        case .permanentlyDelete: return .permanentlyDelete
        case .open, .openInWindow, .reply, .replyAll, .forward, .markAsSpam, .markAsNotSpam,
            .moveToFolder:
            return nil
        }
    }
}

extension CommandItem {
    /// The item as the row menu names it, so a pick from the bar runs through the menu's own
    /// path; `nil` for Sync, which acts on the mailbox rather than on the selection.
    var menuItem: RowMenuItem? {
        guard case .act(let action) = command else { return nil }
        return RowMenuItem(action: action, enabled: enabled, target: target, confirm: confirm)
    }

    /// The label on the bar: the menu's words, except Move, which on a bar opens the picker
    /// rather than reading as a menu entry.
    var title: String {
        switch command {
        case .act(.moveToFolder): return L10n.action_move()
        case .act(let action): return action.title
        case .sync: return L10n.action_refresh()
        }
    }

    /// The item's SF Symbol (`docs/icons.md`).
    var systemImage: String {
        switch command {
        case .act(let action): return action.systemImage
        case .sync: return "arrow.clockwise"
        }
    }
}

/// What a question before deleting for good says about `rows`: one message, one conversation,
/// or how many of either (`docs/command-bar.md`, rule 6).
enum PurgeWording {
    static func message(for rows: [SelectedRow]) -> String {
        let threads = rows.filter { if case .thread = $0 { return true } else { return false } }
        switch (rows.count, threads.count) {
        case (1, 1): return L10n.delete_permanently_conversation()
        case (1, _): return L10n.delete_permanently_message()
        case (let count, 0): return L10n.delete_permanently_message_many(count: count)
        case (let count, _): return L10n.delete_permanently_conversations(count: count)
        }
    }
}

/// A delete for good, waiting on the user's answer.
struct PurgeRequest: Identifiable {
    let id = UUID()
    /// The sentence the question carries, from `PurgeWording`.
    let message: String
    /// The delete itself, run on a yes.
    let run: () -> Void
}

/// Which conversations stay expanded as the selection moves (`docs/row-actions.md`, rule 8).
enum ConversationExpansion {
    /// The expansion key of `thread`: its account-scoped id.
    static func key(_ thread: ThreadRow) -> String { "\(thread.account)/\(thread.threadId)" }

    /// The conversations still expanded once the selection is `selected`, out of `expanded`.
    ///
    /// One row picked keeps open only the conversation it belongs to, the conversation row itself
    /// or one of its messages, so leaving a conversation folds it away, even one opened by its
    /// chevron. Several rows picked, or none, change nothing: adding a conversation to a set
    /// expands nothing, and a selection emptied by an archive is not the user leaving anything.
    static func kept(
        _ expanded: Set<String>, selected: Set<SelectionKey>, rows: [SnapshotRow]
    ) -> Set<String> {
        guard selected.count == 1, let picked = selected.first else { return expanded }
        let holding = rows.compactMap { row -> String? in
            guard case .thread(let thread) = row else { return nil }
            let holds = SelectionKey(row) == picked
                || thread.messages.contains { SelectionKey($0) == picked }
            return holds ? key(thread) : nil
        }
        return expanded.intersection(holding)
    }
}
