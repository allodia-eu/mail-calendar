// What the core's row menu looks like on Apple, and the one rule about expanding a conversation
// (`docs/row-actions.md`). The core decides which items a menu holds, which respond and which
// message each is about; this file gives each action its words and its symbol, and keeps no rule
// of its own about either. Pure, so it is tested without a window.

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
        case .delete: return L10n.action_move_to_trash()
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

/// The selection bar's buttons, read off the core's menu over the selection
/// (`docs/list-selection.md`, rule 5): each toggle names the half that changes something, and a
/// button the menu would draw disabled is disabled on the bar too.
///
/// With nothing selected the core answers no menu, and the bar names the affirmative halves, all
/// disabled: "Mark as unread" over nothing describes an action nobody asked for.
struct SelectionBarActions: Equatable {
    var read: RowMenuItem
    var flag: RowMenuItem
    var archive: RowMenuItem
    var delete: RowMenuItem
    var permanentlyDelete: RowMenuItem

    init(_ menu: RowMenu) {
        // The first item doing one of `actions`, which for a toggle is the half on offer.
        func find(_ actions: RowAction...) -> RowMenuItem {
            menu.sections.lazy.flatMap(\.items).first { actions.contains($0.action) }
                ?? RowMenuItem(action: actions[0], enabled: false, target: nil)
        }
        read = find(.markRead, .markUnread)
        flag = find(.flag, .unflag)
        archive = find(.archive)
        delete = find(.delete)
        permanentlyDelete = find(.permanentlyDelete)
    }
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
