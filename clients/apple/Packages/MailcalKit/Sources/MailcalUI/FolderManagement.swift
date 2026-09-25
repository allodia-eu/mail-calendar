// What the folder pane offers for changing the tree, decided without a window
// (`docs/folder-pane.md`, rules 22 to 28): which menu items a row carries, where a folder may be
// moved, which drops a row takes, and what the dialogs and the notice say.
//
// Eligibility is the core's (`FolderRow::editable`, `acceptsFolders`, `acceptsMessages`,
// `AccountFolderRow::managesFolders`); nothing here decides it again, it only reads it.

import CoreTransferable
import Foundation
import MailcalBindings

/// One item of a folder row's menu (rule 23).
enum FolderMenuItem: Hashable {
    case newFolder, rename, move, delete
}

/// The items a folder row's menu carries, in menu order. Empty for a pending row, and for a row
/// the core allows nothing on; a client then offers no menu at all.
func folderMenuItems(_ row: FolderRow) -> [FolderMenuItem] {
    var items: [FolderMenuItem] = []
    if row.acceptsFolders { items.append(.newFolder) }
    if row.editable { items.append(contentsOf: [.rename, .move, .delete]) }
    return items
}

/// The keys of `key` and every folder beneath it, walking the rows' own `parent`.
func folderSubtree(_ key: String, in folders: [FolderRow]) -> Set<String> {
    var subtree: Set<String> = [key]
    // Each pass adds one level; a tree can be no deeper than it has rows.
    for _ in folders {
        let before = subtree.count
        for row in folders {
            if let parent = row.parent, subtree.contains(parent) { subtree.insert(row.key) }
        }
        if subtree.count == before { break }
    }
    return subtree
}

/// A folder's name with the folders it sits in, `Clients / Acme`, for a list that is flat.
func folderPath(_ row: FolderRow, in folders: [FolderRow]) -> String {
    var parts = [folderLabel(role: row.role, name: row.name)]
    var parent = row.parent
    while let key = parent, parts.count <= folders.count,
          let above = folders.first(where: { $0.key == key }) {
        parts.append(folderLabel(role: above.role, name: above.name))
        parent = above.parent
    }
    return parts.reversed().joined(separator: " / ")
}

/// One place Move to… offers (rule 24): a folder, or the top of the tree when `parent` is `nil`.
struct MoveDestination: Identifiable, Equatable {
    let parent: String?
    let label: String

    var id: String { parent ?? "" }
}

/// Where `row` may be moved to: Top level first, then every folder that takes folders, by path,
/// leaving out the folder itself, everything inside it, and the place it already is.
func moveDestinations(for row: FolderRow, in folders: [FolderRow]) -> [MoveDestination] {
    let excluded = folderSubtree(row.key, in: folders)
    var destinations: [MoveDestination] = []
    if row.parent != nil {
        destinations.append(MoveDestination(parent: nil, label: L10n.folder_move_top_level()))
    }
    for candidate in folders
    where candidate.acceptsFolders && !excluded.contains(candidate.key) && candidate.key != row.parent {
        destinations.append(
            MoveDestination(parent: candidate.key, label: folderPath(candidate, in: folders))
        )
    }
    return destinations
}

/// What a row of the pane carries when it is dragged: a folder, or the messages it stands for.
///
/// Carried as a string under a private prefix, so a drop can tell the pane's own drags from any
/// other text, and so no type needs declaring in each target's Info.plist.
enum PaneDragPayload: Equatable, Sendable, Transferable {
    case folder(account: String, key: String)
    case messages(rows: [DraggedRow])

    private static let prefix = "mailcal-pane:"

    static var transferRepresentation: some TransferRepresentation {
        ProxyRepresentation(exporting: \.token) { token in
            guard let payload = PaneDragPayload(token: token) else {
                throw CocoaError(.coderReadCorrupt)
            }
            return payload
        }
    }

    var token: String {
        let wire: Wire
        switch self {
        case .folder(let account, let key):
            wire = Wire(folder: DraggedRow(account: account, id: key, isThread: false), rows: nil)
        case .messages(let rows): wire = Wire(folder: nil, rows: rows)
        }
        let data = (try? JSONEncoder().encode(wire)) ?? Data()
        return Self.prefix + (String(data: data, encoding: .utf8) ?? "")
    }

    init?(token: String) {
        guard token.hasPrefix(Self.prefix),
              let data = token.dropFirst(Self.prefix.count).data(using: .utf8),
              let wire = try? JSONDecoder().decode(Wire.self, from: data)
        else { return nil }
        if let folder = wire.folder {
            self = .folder(account: folder.account, key: folder.id)
        } else if let rows = wire.rows, !rows.isEmpty {
            self = .messages(rows: rows)
        } else {
            return nil
        }
    }

    private struct Wire: Codable {
        let folder: DraggedRow?
        let rows: [DraggedRow]?
    }
}

/// One dragged message row: a message key, or a conversation's thread id.
struct DraggedRow: Codable, Equatable, Sendable {
    let account: String
    let id: String
    let isThread: Bool

    init(account: String, id: String, isThread: Bool) {
        self.account = account
        self.id = id
        self.isThread = isThread
    }

    init(_ key: SelectionKey) {
        self.init(account: key.account, id: key.id, isThread: key.isThread)
    }

    var selectedRow: SelectedRow {
        isThread ? .thread(account: account, threadId: id) : .message(account: account, key: id)
    }
}

/// Whether `payload` may be dropped on the folder `target` of `account`, or on the account's own
/// row when `target` is `nil` (rules 22 and 24). Neither kind crosses accounts.
func acceptsDrop(
    _ payload: PaneDragPayload,
    onto target: FolderRow?,
    account: String,
    tree: AccountFolderRow?
) -> Bool {
    let folders = tree?.folders ?? []
    switch payload {
    case .folder(let from, let key):
        guard from == account,
              let dragged = folders.first(where: { $0.key == key }), dragged.editable
        else { return false }
        guard let target else {
            // The account row: the top of the tree, which is a move only for a folder not there.
            return tree?.managesFolders == true && dragged.parent != nil
        }
        return target.acceptsFolders
            && !folderSubtree(key, in: folders).contains(target.key)
            && dragged.parent != target.key
    case .messages(let rows):
        guard let target, target.acceptsMessages else { return false }
        return rows.contains { $0.account == account }
    }
}

/// What a name dialog says under its field, or `nil` when there is nothing to say. An empty field
/// says nothing: the disabled button already does.
func folderNameProblem(_ check: FolderNameCheck) -> String? {
    switch check {
    case .valid, .empty: nil
    case .surrounded: L10n.folder_name_surrounded()
    case .control: L10n.folder_name_control()
    case .separator: L10n.folder_name_separator()
    case .taken: L10n.folder_name_taken()
    }
}

/// The delete confirmation's words, by whether the folder is already in Trash (rule 26).
struct FolderDeleteCopy: Equatable {
    let title: String
    let message: String
    let confirm: String
}

func folderDeleteCopy(_ row: FolderRow) -> FolderDeleteCopy {
    let name = folderLabel(role: row.role, name: row.name)
    return row.inTrash
        ? FolderDeleteCopy(
            title: L10n.folder_delete_permanent_title(name: name),
            message: L10n.folder_delete_permanent_message(),
            confirm: L10n.action_delete_permanently()
        )
        : FolderDeleteCopy(
            title: L10n.folder_delete_title(name: name),
            message: L10n.folder_delete_message(),
            confirm: L10n.action_move_to_trash()
        )
}

/// The line a refused change stands on the pane with (rule 28).
func folderNoticeText(_ notice: FolderNotice) -> String {
    switch notice.problem {
    case .changedElsewhere: L10n.folder_notice_changed(name: notice.folder)
    case .refused: L10n.folder_notice_refused(name: notice.folder)
    }
}

/// Which name dialog is open: making a folder, or renaming one.
enum FolderNameRequest: Identifiable, Equatable {
    case create(account: String, parent: String?)
    case rename(account: String, folder: FolderRow)

    var id: String {
        switch self {
        case .create(let account, let parent): "create/\(account)/\(parent ?? "")"
        case .rename(let account, let folder): "rename/\(account)/\(folder.key)"
        }
    }

    var account: String {
        switch self {
        case .create(let account, _), .rename(let account, _): account
        }
    }

    /// The folder the new name is to sit inside, which the name is checked against.
    var parent: String? {
        switch self {
        case .create(_, let parent): parent
        case .rename(_, let folder): folder.parent
        }
    }

    var renaming: String? {
        if case .rename(_, let folder) = self { return folder.key }
        return nil
    }

    var title: String {
        switch self {
        case .create: L10n.folder_new_title()
        case .rename: L10n.folder_rename_title()
        }
    }

    var confirm: String {
        switch self {
        case .create: L10n.action_create()
        case .rename: L10n.action_save()
        }
    }

    var initialName: String {
        if case .rename(_, let folder) = self { return folder.name }
        return ""
    }

    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
}

/// Which folder sheet is open: a name dialog, or Move to….
enum FolderSheet: Identifiable, Equatable {
    case name(FolderNameRequest)
    case move(FolderTarget)

    var id: String {
        switch self {
        case .name(let request): "name/\(request.id)"
        case .move(let target): "move/\(target.id)"
        }
    }
}

/// A folder row a dialog was opened for, with the account whose tree it is in.
struct FolderTarget: Identifiable, Equatable {
    let account: String
    let folder: FolderRow

    var id: String { "\(account)/\(folder.key)" }

    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
}
