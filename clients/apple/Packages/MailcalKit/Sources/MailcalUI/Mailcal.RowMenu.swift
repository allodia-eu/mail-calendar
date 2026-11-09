// The menu a secondary click or a long press raises over the message list, drawn as the core
// answers it (`docs/row-actions.md`): one row's own menu, a conversation's included, or the menu
// over the selection that row belongs to. This file draws it and runs what is picked; which items
// there are, which respond and which message each is about are the core's.

import MailcalBindings
import SwiftUI

extension ContentView {
    /// The menu over `keys`. The toolkit hands over the selection when the row under the pointer
    /// is in it, and that row alone when it is not (`docs/list-selection.md`, rule 12).
    @ViewBuilder
    func rowMenu(_ keys: Set<SelectionKey>) -> some View {
        let menu = model.rowMenu(rows(of: keys))
        ForEach(Array(menu.sections.enumerated()), id: \.offset) { index, section in
            if index > 0 { Divider() }
            ForEach(Array(section.items.enumerated()), id: \.offset) { _, item in
                rowMenuButton(item, over: keys)
            }
        }
    }

    /// `keys` as the core names rows, a message picked inside a conversation that is itself
    /// picked left out, since the conversation already stands for it.
    private func rows(of keys: Set<SelectionKey>) -> [SelectedRow] {
        MailSelection(keys: keys).selectedRows(expanded: expandedConversations)
    }

    private func rowMenuButton(_ item: RowMenuItem, over keys: Set<SelectionKey>) -> some View {
        let available = item.enabled
            && (item.action != .moveToFolder || canAskMessageMove(Array(keys)))
        return Button(role: item.confirm ? .destructive : nil) {
            perform(item, over: keys)
        } label: {
            Label(item.action.title, systemImage: item.action.systemImage)
        }
        .disabled(!available)
    }

    /// Runs `item`: on its one message when it names one, otherwise over the rows the menu (or
    /// the bar) was raised on. Where the core set `confirm` it asks first, naming what goes
    /// (`docs/command-bar.md`, rule 6); the bar and the Delete key run through here too, so no
    /// route deletes for good without the question.
    func perform(_ item: RowMenuItem, over keys: Set<SelectionKey>) {
        if item.confirm {
            let rows = item.target.map { [SelectedRow.message(account: $0.account, key: $0.key)] }
                ?? rows(of: keys)
            pendingPurge = PurgeRequest(message: PurgeWording.message(for: rows)) {
                run(item, over: keys)
            }
        } else {
            run(item, over: keys)
        }
    }

    private func run(_ item: RowMenuItem, over keys: Set<SelectionKey>) {
        if item.action == .moveToFolder {
            askMessageMove(Array(keys))
        } else if let target = item.target {
            perform(item.action, on: target)
        } else if let action = item.action.bulkAction {
            act(action, on: keys)
        }
    }

    private func perform(_ action: RowAction, on message: MenuMessage) {
        let (account, key) = (message.account, message.key)
        // Taking away the message the pane is showing moves the pane on, as the pane's own
        // Archive and Delete do (`docs/reading-actions.md`, "Where the pane goes next"): asked
        // before the dispatch, while the row it is relative to is still listed.
        let removes = [RowAction.archive, .delete, .permanentlyDelete, .markAsSpam, .markAsNotSpam]
            .contains(action)
        let opened = openedMessage.flatMap { $0.account == account && $0.key == key ? $0 : nil }
        let next = removes ? opened.map(stopAfterRemoving) : nil
        defer { if removes, opened != nil { settleReadingPane(next ?? nil) } }
        switch action {
        case .open: openAsClicked(message)
        case .openInWindow: openMenuTargetInWindow(message)
        case .reply, .replyAll: beginReply(account, key, all: action == .replyAll)
        case .forward: beginForward(account, key)
        case .markRead, .markUnread: model.markRead(account, key, action == .markRead)
        case .flag, .unflag: model.setFlagged(account, key, action == .flag)
        case .markAsSpam: model.markAsSpam(account, key)
        case .markAsNotSpam: model.markAsNotSpam(account, key)
        case .archive: model.archive(account, key)
        case .delete: model.delete(account, key)
        case .permanentlyDelete: model.permanentlyDelete(account, key)
        case .moveToFolder: break
        }
    }

    /// The batch over `keys`, tidied up as the bar's is: through the bar's own path when the menu
    /// was raised on the selection, otherwise for those rows alone, letting the reading pane go
    /// when its message is leaving.
    private func act(_ action: BulkAction, on keys: Set<SelectionKey>) {
        if keys == selection.keys {
            actOnSelection(action)
            return
        }
        let closesReading = action.removesRows && holdsOpenMessage(keys)
        model.actOnSelection(rows(of: keys), action)
        if closesReading { clearOpenedMessage() }
    }

    /// Opens a message the core named as clicking its row would: a flat row as itself, a
    /// conversation's message by expanding its conversation onto it, a draft into its composer.
    func openAsClicked(_ target: MenuMessage) {
        if let (thread, message) = conversationMessage(target) {
            expandedThreads.insert(threadKey(thread))
            openThreadMessage(thread, message)
        } else if let flat = flatRow(target) {
            open(flat)
        }
    }

    private func openMenuTargetInWindow(_ target: MenuMessage) {
        #if os(macOS)
        if let (thread, message) = conversationMessage(target) {
            openInWindow(thread, message)
        } else if let flat = flatRow(target) {
            openInWindow(flat)
        }
        #endif
    }

    private func conversationMessage(_ target: MenuMessage) -> (ThreadRow, ThreadMessage)? {
        for case .thread(let thread) in visibleRows {
            if let message = thread.messages.first(where: {
                $0.account == target.account && $0.key == target.key
            }) {
                return (thread, message)
            }
        }
        return nil
    }

    private func flatRow(_ target: MenuMessage) -> FlatRow? {
        for case .flat(let row) in visibleRows where row.account == target.account
            && row.key == target.key {
            return row
        }
        return nil
    }
}
