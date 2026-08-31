// The folder pane's client-side half on Apple (Mailcal.Layout.swift), per docs/folder-pane.md.
//
// Two mappings the pane cannot get from the core, because the core has no locale and no icon set:
// what a folder is CALLED (rule 12) and which glyph it takes (rule 9).
//
// And the identity of a row. A folder key is unique only within its account, so with every
// account's tree on screen at once the pane holds several rows keyed `inbox`. Two rows sharing an
// identity in one SwiftUI `List` are one row: the second account's Inbox drew the first account's
// unread count, with the right folders under the right accounts and nothing else out of place.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct FolderPaneTests {

    private func folder(_ key: String, _ name: String, _ role: FolderRole?) -> FolderRow {
        FolderRow(key: key, name: name, role: role, unread: 0)
    }

    // MARK: rule 12, a known folder is called what we call it

    @Test func aKnownFolderTakesTheAppsNameOverTheServers() {
        // What servers really call them: the one name IMAP mandates, and Exchange's words.
        #expect(folderLabel(role: .inbox, name: "INBOX") == L10n.folder_inbox())
        #expect(folderLabel(role: .trash, name: "Deleted Items") == L10n.folder_trash())
        #expect(folderLabel(role: .sent, name: "Sent Items") == L10n.folder_sent())
        #expect(folderLabel(role: .junk, name: "Junk Mail") == L10n.folder_junk())
        #expect(folderLabel(role: .drafts, name: "Concepten") == L10n.folder_drafts())
        #expect(folderLabel(role: .archive, name: "Archief 2024") == L10n.folder_archive())
    }

    @Test func aFolderTheUserMadeKeepsItsName() {
        #expect(folderLabel(role: nil, name: "Tenders") == "Tenders")
        // `.other` too: the core collapses flagged, important and all-mail into it, so no one word
        // is honest for all three.
        #expect(folderLabel(role: .other, name: "[Gmail]/All Mail") == "[Gmail]/All Mail")
    }

    // MARK: rule 9, the icon comes from the role, never the name

    @Test func eachRoleTakesItsOwnGlyphAndEverythingElseTakesThePlainFolder() {
        let roles: [FolderRole] = [.inbox, .drafts, .sent, .archive, .junk, .trash]
        let glyphs = roles.map { folderIcon($0) }
        #expect(Set(glyphs).count == roles.count, "a role that shares its glyph is a role the pane cannot tell apart")
        #expect(folderIcon(nil) == "folder")
        #expect(folderIcon(.other) == "folder")
    }

    // MARK: every row in the pane is its own row

    @Test func twoAccountsInboxesAreTwoRows() {
        let mine = SidebarFolder(account: "work", folder: folder("inbox", "INBOX", .inbox))
        let theirs = SidebarFolder(account: "home", folder: folder("inbox", "INBOX", .inbox))

        #expect(mine.id != theirs.id)
    }

    @Test func oneAccountsFoldersStayDistinctFromEachOther() {
        let account = "work"
        let ids = [
            SidebarFolder(account: account, folder: folder("inbox", "INBOX", .inbox)),
            SidebarFolder(account: account, folder: folder("sent", "Sent Items", .sent)),
        ].map(\.id)

        #expect(Set(ids).count == ids.count)
    }
}
