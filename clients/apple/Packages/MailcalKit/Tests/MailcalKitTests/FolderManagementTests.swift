// Changing the folder tree from the pane (`docs/folder-pane.md`, rules 22 to 28), decided without
// a window: which menu items a row carries, where Move to… offers, which drops a row takes, and
// what the dialogs and the notice say.
//
// Eligibility is the core's; these check the pane reads it and adds no rule of its own, and that
// a folder can never be offered a place inside itself.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct FolderManagementTests {
    private func row(
        _ key: String,
        _ name: String,
        parent: String? = nil,
        role: FolderRole? = nil,
        pending: Bool = false,
        inTrash: Bool = false,
        editable: Bool = true,
        acceptsFolders: Bool = true,
        acceptsMessages: Bool = true
    ) -> FolderRow {
        FolderRow(
            key: key, name: name, role: role, unread: 0, parent: parent, depth: 0,
            hasChildren: false, expanded: false, visible: true, pending: pending,
            inTrash: inTrash, editable: editable, acceptsFolders: acceptsFolders,
            acceptsMessages: acceptsMessages
        )
    }

    /// Inbox, Junk, and `Work` holding `2024`, which holds `Q1`.
    private var tree: [FolderRow] {
        [
            row("inbox", "INBOX", role: .inbox, editable: false),
            row("junk", "Junk", role: .junk, editable: false, acceptsFolders: false, acceptsMessages: false),
            row("work", "Work"),
            row("w2024", "2024", parent: "work"),
            row("q1", "Q1", parent: "w2024"),
        ]
    }

    private func account(manages: Bool = true) -> AccountFolderRow {
        AccountFolderRow(accountId: "acct-1", folders: tree, managesFolders: manages)
    }

    // MARK: rule 23, the menu offers what the row allows

    @Test func aFolderTheUserMadeOffersEveryItemAndARoleFolderOnlyANewFolder() {
        #expect(folderMenuItems(tree[2]) == [.newFolder, .rename, .move, .delete])
        #expect(folderMenuItems(tree[0]) == [.newFolder])
        #expect(folderMenuItems(tree[1]).isEmpty, "a row allowing nothing gets no menu")
    }

    @Test func aPendingRowOffersNothing() {
        let waiting = row("new", "Travel", pending: true, editable: false, acceptsFolders: false, acceptsMessages: false)
        #expect(folderMenuItems(waiting).isEmpty)
    }

    // MARK: rule 24, Move to… never offers a place inside the folder itself

    @Test func moveToLeavesOutTheFolderItsBranchAndWhereItAlreadyIs() {
        let destinations = moveDestinations(for: tree[3], in: tree)

        #expect(destinations.first == MoveDestination(parent: nil, label: L10n.folder_move_top_level()))
        let parents = destinations.map(\.parent)
        #expect(!parents.contains("w2024"), "not into itself")
        #expect(!parents.contains("q1"), "not into its own subfolder")
        #expect(!parents.contains("work"), "not where it already is")
        #expect(!parents.contains("junk"), "only folders that take folders")
        #expect(parents.contains("inbox"))
    }

    @Test func aTopLevelFolderIsNotOfferedTheTopLevel() {
        let parents = moveDestinations(for: tree[2], in: tree).map(\.parent)
        #expect(!parents.contains(nil))
    }

    @Test func aDestinationIsNamedByItsPath() {
        let q1 = moveDestinations(for: row("x", "X"), in: tree).first { $0.parent == "q1" }
        #expect(q1?.label == "Work / 2024 / Q1")
        let inbox = moveDestinations(for: row("x", "X"), in: tree).first { $0.parent == "inbox" }
        #expect(inbox?.label == L10n.folder_inbox(), "a role folder takes the app's word")
    }

    // MARK: rules 22 and 24, drops

    @Test func aFolderDropsOntoAFolderThatTakesOneButNeverIntoItsOwnBranch() {
        let work = PaneDragPayload.folder(account: "acct-1", key: "work")
        #expect(acceptsDrop(work, onto: tree[0], account: "acct-1", tree: account()))
        #expect(!acceptsDrop(work, onto: tree[3], account: "acct-1", tree: account()))
        #expect(!acceptsDrop(work, onto: tree[1], account: "acct-1", tree: account()))
        #expect(!acceptsDrop(work, onto: tree[0], account: "acct-2", tree: account()), "never across accounts")
    }

    @Test func theAccountRowTakesAFolderToTheTopOnlyWhenItManagesFolders() {
        let nested = PaneDragPayload.folder(account: "acct-1", key: "w2024")
        #expect(acceptsDrop(nested, onto: nil, account: "acct-1", tree: account()))
        #expect(!acceptsDrop(nested, onto: nil, account: "acct-1", tree: account(manages: false)))
        let top = PaneDragPayload.folder(account: "acct-1", key: "work")
        #expect(!acceptsDrop(top, onto: nil, account: "acct-1", tree: account()), "already at the top")
    }

    @Test func aRoleFolderCannotBeDragged() {
        let inbox = PaneDragPayload.folder(account: "acct-1", key: "inbox")
        #expect(!acceptsDrop(inbox, onto: tree[2], account: "acct-1", tree: account()))
    }

    @Test func mailDropsOnlyWhereTheRowTakesMailAndInItsOwnAccount() {
        let mail = PaneDragPayload.messages(rows: [DraggedRow(account: "acct-1", id: "m1", isThread: false)])
        #expect(acceptsDrop(mail, onto: tree[2], account: "acct-1", tree: account()))
        #expect(!acceptsDrop(mail, onto: tree[1], account: "acct-1", tree: account()), "spam is a report")
        #expect(!acceptsDrop(mail, onto: nil, account: "acct-1", tree: account()))
        #expect(!acceptsDrop(mail, onto: tree[2], account: "acct-2", tree: account()))
    }

    @Test func aDragSurvivesItsTripAsTextAndOtherTextIsNotOne() {
        let payloads: [PaneDragPayload] = [
            .folder(account: "acct-1", key: "Work/2024"),
            .messages(rows: [
                DraggedRow(account: "acct-1", id: "m1", isThread: false),
                DraggedRow(account: "acct-2", id: "t9", isThread: true),
            ]),
        ]
        for payload in payloads {
            #expect(PaneDragPayload(token: payload.token) == payload)
        }
        #expect(PaneDragPayload(token: "Work") == nil)
        #expect(
            DraggedRow(account: "acct-2", id: "t9", isThread: true).selectedRow
                == .thread(account: "acct-2", threadId: "t9")
        )
    }

    // MARK: rules 25, 26 and 28, what the dialogs and the notice say

    @Test func eachRefusedNameHasItsOwnLineAndAnEmptyFieldSaysNothing() {
        #expect(folderNameProblem(.valid) == nil)
        #expect(folderNameProblem(.empty) == nil)
        let lines = [FolderNameCheck.surrounded, .control, .separator, .taken].compactMap(folderNameProblem)
        #expect(Set(lines).count == 4)
    }

    @Test func deletingInTrashIsWordedAsPermanent() {
        let outside = folderDeleteCopy(row("work", "Work"))
        #expect(outside.title == L10n.folder_delete_title(name: "Work"))
        #expect(outside.confirm == L10n.action_move_to_trash())
        let inside = folderDeleteCopy(row("old", "Old", inTrash: true))
        #expect(inside.title == L10n.folder_delete_permanent_title(name: "Old"))
        #expect(inside.confirm == L10n.action_delete_permanently())
    }

    @Test func theNoticeSaysWhyTheChangeDidNotHappen() {
        let changed = FolderNotice(account: "acct-1", folder: "Work", action: .rename, problem: .changedElsewhere)
        #expect(folderNoticeText(changed) == L10n.folder_notice_changed(name: "Work"))
        let refused = FolderNotice(account: "acct-1", folder: "Work", action: .create, problem: .refused)
        #expect(folderNoticeText(refused) == L10n.folder_notice_refused(name: "Work"))
    }

    @Test func aRenameIsCheckedBesideItselfAndACreateInsideItsParent() {
        let rename = FolderNameRequest.rename(account: "acct-1", folder: tree[3])
        #expect(rename.parent == "work")
        #expect(rename.renaming == "w2024")
        #expect(rename.initialName == "2024")
        let create = FolderNameRequest.create(account: "acct-1", parent: "work")
        #expect(create.parent == "work")
        #expect(create.renaming == nil)
        #expect(create.initialName.isEmpty)
    }
}
