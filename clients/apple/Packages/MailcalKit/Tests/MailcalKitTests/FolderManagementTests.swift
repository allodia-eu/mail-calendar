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
        depth: UInt32 = 0,
        role: FolderRole? = nil,
        pending: Bool = false,
        inTrash: Bool = false,
        editable: Bool = true,
        acceptsFolders: Bool = true,
        acceptsMessages: Bool = true
    ) -> FolderRow {
        FolderRow(
            key: key, name: name, role: role, unread: 0, parent: parent, depth: depth,
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
            row("w2024", "2024", parent: "work", depth: 1),
            row("q1", "Q1", parent: "w2024", depth: 2),
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

    /// The keys of `targets`, Top level as `nil`.
    private func keys(_ targets: [MoveTarget]) -> [String?] { targets.map(\.key) }

    @Test func moveToLeavesOutTheFolderItsBranchAndWhereItAlreadyIs() {
        let targets = moveTargets(for: tree[3], in: tree)

        let topLevel = L10n.folder_move_top_level()
        #expect(
            targets.first
                == MoveTarget(key: nil, name: topLevel, role: nil, indent: 0, enabled: true, label: topLevel)
        )
        let expected: [String?] = [nil, "inbox"]
        #expect(keys(targets) == expected, "not into itself, its subfolder, where it is, or Junk")
    }

    @Test func aTopLevelFolderSeesTopLevelAsTheRootButCannotChooseIt() {
        let targets = moveTargets(for: tree[2], in: tree)
        let expected: [String?] = [nil, "inbox"]
        #expect(keys(targets) == expected, "nothing of the folder's own branch, which takes folders")
        #expect(targets.first?.enabled == false, "it is already there")
        #expect(targets.last?.indent == 1, "the folders still sit inside it")
    }

    @Test func folderMoveDrawsThePanesTreeOneStepInsideTopLevel() {
        let targets = moveTargets(for: row("x", "X", parent: "q1", depth: 3), in: tree)

        let expectedKeys: [String?] = [nil, "inbox", "work", "w2024"]
        #expect(keys(targets) == expectedKeys, "Q1 is where it already is, and holds nothing")
        #expect(targets.map(\.indent) == [0, 1, 1, 2])
        #expect(targets.map(\.name) == [L10n.folder_move_top_level(), L10n.folder_inbox(), "Work", "2024"])
        let expectedRoles: [FolderRole?] = [nil, .inbox, nil, nil]
        #expect(targets.map(\.role) == expectedRoles)
        #expect(targets.allSatisfy { $0.enabled })
    }

    @Test func aFolderThatIsNoDestinationStaysDisabledOnlyAboveOneThatIs() {
        var folders = tree
        folders.append(row("shared", "Shared", editable: false, acceptsFolders: false, acceptsMessages: false))
        folders.append(row("team", "Team", parent: "shared", depth: 1))

        let targets = moveTargets(for: row("x", "X", parent: "work", depth: 1), in: folders)
        let work = targets.first { $0.key == "work" }
        #expect(work?.enabled == false, "where it already is, kept above 2024")
        #expect(targets.first { $0.key == "w2024" }?.enabled == true)
        let shared = targets.first { $0.key == "shared" }
        #expect(shared?.enabled == false, "takes no folder, kept above Team")
        #expect(shared?.indent == 1)
        #expect(targets.first { $0.key == "team" }?.indent == 2)
        #expect(!keys(targets).contains("junk"), "takes nothing and holds nothing")
    }

    @Test func aRowShowsItsOwnNameAndIsReadAsItsPath() {
        let targets = moveTargets(for: row("x", "X"), in: tree)
        let q1 = targets.first { $0.key == "q1" }
        #expect(q1?.name == "Q1")
        #expect(q1?.label == "Work / 2024 / Q1")
        let inbox = targets.first { $0.key == "inbox" }
        #expect(inbox?.name == L10n.folder_inbox(), "a role folder takes the app's word")
        #expect(inbox?.label == L10n.folder_inbox())
        #expect(targets.first?.label == L10n.folder_move_top_level())
    }

    // MARK: rule 24, Move to folder… on a message row

    @Test func mailIsOfferedEveryFolderThatTakesItButTheOneTheListShows() {
        var folders = tree
        folders.append(row("drafts", "Drafts", role: .drafts, editable: false, acceptsFolders: false, acceptsMessages: false))
        folders.append(row("new", "Travel", pending: true, editable: false, acceptsFolders: false, acceptsMessages: false))
        let mailbox = AccountFolderRow(accountId: "acct-1", folders: folders, managesFolders: true)

        let targets = messageTargets(in: mailbox, showing: "inbox")
        let expected: [String?] = ["work", "w2024", "q1"]
        #expect(keys(targets) == expected, "no Top level: mail always sits in a folder")
        #expect(targets.map(\.indent) == [0, 1, 2], "each folder at its own depth")
        #expect(!keys(targets).contains("junk"), "spam is a report")
        #expect(!keys(targets).contains("drafts"))
        #expect(!keys(targets).contains("new"), "a folder still being made takes nothing")
        #expect(!keys(targets).contains("inbox"), "not the folder the mail is already in")
        #expect(keys(messageTargets(in: mailbox, showing: nil)).contains("inbox"))
        #expect(
            messageTargets(in: mailbox, showing: nil).first { $0.key == "q1" }?.label
                == "Work / 2024 / Q1"
        )
    }

    @Test func theFolderOnScreenStaysAsTheParentOfOneThatTakesMail() {
        let mailbox = AccountFolderRow(accountId: "acct-1", folders: tree, managesFolders: true)

        let fromWork = messageTargets(in: mailbox, showing: "work")
        #expect(fromWork.first { $0.key == "work" }?.enabled == false)
        #expect(fromWork.first { $0.key == "w2024" }?.enabled == true)
        let fromQ1: [String?] = ["inbox", "work", "w2024"]
        #expect(keys(messageTargets(in: mailbox, showing: "q1")) == fromQ1, "it holds nothing")
    }

    @Test func noFolderLeftToOfferMeansNoItem() {
        let only = AccountFolderRow(
            accountId: "acct-1",
            folders: [row("inbox", "INBOX", role: .inbox, editable: false), tree[1]],
            managesFolders: true
        )
        #expect(!offersMessageMove(in: only, showing: "inbox"))
        #expect(offersMessageMove(in: only, showing: nil))
        #expect(offersMessageMove(in: account(manages: false), showing: nil), "filing mail needs no folder writes")
    }

    /// One message row of `account`, as a menu names it.
    private func mail(_ account: String, _ key: String) -> SelectionKey {
        let swatch = Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB")
        return SelectionKey(.flat(row: FlatRow(
            account: account, key: key, subject: "Subject", from: "sender",
            avatar: Avatar(initials: "S", light: swatch, dark: swatch, imagePath: nil),
            date: "2026-07-20", unread: false, flagged: false, hasAttachment: false, preview: ""
        )))
    }

    @Test func mailOfTwoAccountsIsOfferedNoFolder() {
        #expect(soleAccount(of: [mail("acct-1", "m1")]) == "acct-1")
        #expect(soleAccount(of: [mail("acct-1", "m1"), mail("acct-1", "m3")]) == "acct-1")
        #expect(soleAccount(of: [mail("acct-1", "m1"), mail("acct-2", "m2")]) == nil, "mail never crosses accounts")
        #expect(soleAccount(of: []) == nil)
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
