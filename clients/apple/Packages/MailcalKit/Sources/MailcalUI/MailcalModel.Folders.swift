// The folder tree's changes, as the pane's menus, dialogs and drops call them
// (`docs/folder-pane.md`, "Changing the tree").
//
// Every change names its folder by account **and** key (rule 14). Which change a row may offer
// is the core's answer on the row itself; the core also ignores one the row did not offer.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// One account's tree as the snapshot gave it, every row, visible or not.
    func accountFolderRow(for account: String) -> AccountFolderRow? {
        accountFolders.first { $0.accountId == account }
    }

    func createFolder(account: String, parent: String?, name: String) {
        app?.dispatch(intent: .folders(intent: .create(account: account, parent: parent, name: name)))
    }

    func renameFolder(account: String, key: String, name: String) {
        app?.dispatch(intent: .folders(intent: .rename(account: account, key: key, name: name)))
    }

    /// Moves a folder into `parent`, or to the top of its account's tree when `parent` is `nil`.
    func moveFolder(account: String, key: String, parent: String?) {
        app?.dispatch(intent: .folders(intent: .move(account: account, key: key, parent: parent)))
    }

    /// Into Trash, or for good when the folder is already there; the caller has confirmed.
    func deleteFolder(account: String, key: String) {
        app?.dispatch(intent: .folders(intent: .delete(account: account, key: key)))
    }

    /// Files dropped rows in a folder. Rows of another account stay where they are.
    func moveMessages(_ rows: [SelectedRow], account: String, key: String) {
        guard !rows.isEmpty else { return }
        app?.dispatch(intent: .folders(intent: .moveMessages(rows: rows, account: account, key: key)))
    }

    func dismissFolderNotice() {
        app?.dispatch(intent: .folders(intent: .dismissNotice))
    }

    /// Whether `name` can be given to a folder inside `parent`, asked while the user types.
    /// A store read in the core, short enough for a keystroke.
    func checkFolderName(
        account: String, parent: String?, name: String, renaming: String?
    ) -> FolderNameCheck {
        app?.checkFolderName(account: account, parent: parent, name: name, renaming: renaming)
            ?? .empty
    }
}
