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
        return Button(role: item.action == .permanentlyDelete ? .destructive : nil) {
            perform(item, over: keys)
        } label: {
            Label(item.action.title, systemImage: item.action.systemImage)
        }
        .disabled(!available)
    }

    /// Runs `item`: on its one message when it names one, otherwise over the rows the menu was
    /// raised on.
    private func perform(_ item: RowMenuItem, over keys: Set<SelectionKey>) {
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
        switch action {
        case .open: openMenuTarget(message)
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

    /// Opens the message an item names: a flat row as itself, a conversation's message by
    /// expanding its conversation onto it, as selecting the conversation would.
    private func openMenuTarget(_ target: MenuMessage) {
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
