// Changing the folder tree from the pane: each folder row's menu, drag and drop, the dialogs
// they open, and the notice a refused change leaves (`docs/folder-pane.md`, rules 22 to 29).
//
// The decisions are FolderManagement.swift's, testable without a window; this file only draws
// them. Dragging is offered where the pane stands beside the list (macOS, and an iPad with a
// reading pane), never in the iPhone's drawer, which covers the list a drag would start from;
// there Move to… is the route (rule 24).

import MailcalBindings
import SwiftUI

extension View {
    /// Applies `transform` only when `condition` holds, for a modifier that has no "off" form.
    @ViewBuilder
    func when<T: View>(_ condition: Bool, _ transform: (Self) -> T) -> some View {
        if condition { transform(self) } else { self }
    }
}

/// A folder row's menu, its waiting state, and its part in drag and drop.
struct FolderRowActions: ViewModifier {
    let model: MailboxModel
    let account: String
    let folder: FolderRow
    let dragEnabled: Bool
    @Binding var sheet: FolderSheet?
    @Binding var deleting: FolderTarget?
    @State private var targeted = false

    func body(content: Content) -> some View {
        let items = folderMenuItems(folder)
        // Built here rather than inside `draggable`'s closure, so the closure holds a value and
        // not this modifier.
        let dragged = PaneDragPayload.folder(account: account, key: folder.key)
        content
            // Waiting for the server (rule 27): dimmed, said aloud, and offering nothing.
            .opacity(folder.pending ? 0.5 : 1)
            .when(folder.pending) { row in
                row.help(L10n.folder_pending()).accessibilityValue(L10n.folder_pending())
            }
            .when(!items.isEmpty) { row in row.contextMenu { menu(items) } }
            .when(dragEnabled && folder.editable) { row in
                row.draggable(dragged)
            }
            .when(dragEnabled && (folder.acceptsFolders || folder.acceptsMessages)) { row in
                row.dropDestination(for: PaneDragPayload.self) { dropped, _ in
                    drop(dropped.first)
                } isTargeted: { over in targeted = over }
            }
            .background(targeted ? Color.accentColor.opacity(0.25) : Color.clear)
    }

    @ViewBuilder
    private func menu(_ items: [FolderMenuItem]) -> some View {
        let target = FolderTarget(account: account, folder: folder)
        ForEach(items, id: \.self) { item in
            switch item {
            case .newFolder:
                Button {
                    sheet = .name(.create(account: account, parent: folder.key))
                } label: {
                    Label(L10n.folder_action_new(), systemImage: "folder.badge.plus")
                }
            case .rename:
                Button { sheet = .name(.rename(account: account, folder: folder)) } label: {
                    Label(L10n.folder_action_rename(), systemImage: "pencil")
                }
            case .move:
                Button { sheet = .move(target) } label: {
                    Label(L10n.folder_action_move(), systemImage: "folder")
                }
            case .delete:
                Button(role: .destructive) { deleting = target } label: {
                    Label(L10n.folder_action_delete(), systemImage: "trash")
                }
            }
        }
    }

    private func drop(_ payload: PaneDragPayload?) -> Bool {
        guard let payload,
              acceptsDrop(
                  payload, onto: folder, account: account,
                  tree: model.accountFolderRow(for: account)
              )
        else { return false }
        switch payload {
        case .folder(_, let key):
            model.moveFolder(account: account, key: key, parent: folder.key)
        case .messages(let rows):
            model.moveMessages(
                rows.filter { $0.account == account }.map(\.selectedRow),
                account: account,
                key: folder.key
            )
        }
        return true
    }
}

/// An account row as a drop target: a folder of its own dropped there moves to the top of the tree.
struct AccountRowFolderDrop: ViewModifier {
    let model: MailboxModel
    let account: String
    let dragEnabled: Bool
    @State private var targeted = false

    func body(content: Content) -> some View {
        content
            .when(dragEnabled && model.accountFolderRow(for: account)?.managesFolders == true) { row in
                row.dropDestination(for: PaneDragPayload.self) { dropped, _ in
                    guard let payload = dropped.first,
                          acceptsDrop(
                              payload, onto: nil, account: account,
                              tree: model.accountFolderRow(for: account)
                          ),
                          case .folder(_, let key) = payload
                    else { return false }
                    model.moveFolder(account: account, key: key, parent: nil)
                    return true
                } isTargeted: { over in targeted = over }
            }
            .background(targeted ? Color.accentColor.opacity(0.25) : Color.clear)
    }
}

/// A message row as a drag source: the whole selection when the row is in it, else the row alone.
struct MessageDragSource: ViewModifier {
    let enabled: Bool
    let payload: () -> PaneDragPayload

    func body(content: Content) -> some View {
        let dragged = payload()
        content.when(enabled) { row in row.draggable(dragged) }
    }
}

/// The folder sheets and the delete confirmation, on the shell so every layout reaches them.
struct FolderDialogs: ViewModifier {
    let model: MailboxModel
    @Binding var sheet: FolderSheet?
    @Binding var deleting: FolderTarget?

    func body(content: Content) -> some View {
        content
            .sheet(item: $sheet) { sheet in
                switch sheet {
                case .name(let request):
                    FolderNameSheet(
                        request: request,
                        check: { name in
                            model.checkFolderName(
                                account: request.account, parent: request.parent,
                                name: name, renaming: request.renaming
                            )
                        },
                        submit: { name in submit(request, name) }
                    )
                case .move(let target):
                    FolderMoveSheet(
                        target: target,
                        destinations: moveDestinations(
                            for: target.folder,
                            in: model.accountFolderRow(for: target.account)?.folders ?? []
                        ),
                        move: { parent in
                            model.moveFolder(account: target.account, key: target.folder.key, parent: parent)
                        }
                    )
                }
            }
            // An alert, not a confirmation dialog, for the reason the remove-account prompt is one
            // (Mailcal.swift): an iPad popover drops the Cancel button.
            .alert(
                deleting.map { folderDeleteCopy($0.folder).title } ?? "",
                isPresented: Binding(
                    get: { deleting != nil },
                    set: { if !$0 { deleting = nil } }
                ),
                presenting: deleting
            ) { target in
                Button(folderDeleteCopy(target.folder).confirm, role: .destructive) {
                    model.deleteFolder(account: target.account, key: target.folder.key)
                }
                Button(L10n.action_cancel(), role: .cancel) {}
            } message: { target in
                Text(folderDeleteCopy(target.folder).message)
            }
    }

    private func submit(_ request: FolderNameRequest, _ name: String) {
        switch request {
        case .create(let account, let parent):
            model.createFolder(account: account, parent: parent, name: name)
        case .rename(let account, let folder):
            model.renameFolder(account: account, key: folder.key, name: name)
        }
    }
}

/// The name dialog. A sheet rather than an alert because it carries a live line under the field
/// saying why a name cannot be used (rule 25).
struct FolderNameSheet: View {
    let request: FolderNameRequest
    let check: (String) -> FolderNameCheck
    let submit: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var name: String
    @FocusState private var focused: Bool

    init(
        request: FolderNameRequest,
        check: @escaping (String) -> FolderNameCheck,
        submit: @escaping (String) -> Void
    ) {
        self.request = request
        self.check = check
        self.submit = submit
        _name = State(initialValue: request.initialName)
    }

    var body: some View {
        let verdict = check(name)
        // A rename to the name it already has is not a change.
        let usable = verdict == .valid && name != request.initialName
        VStack(alignment: .leading, spacing: 12) {
            Text(request.title).font(.headline)
            TextField(L10n.folder_name_label(), text: $name)
                .textFieldStyle(.roundedBorder)
                .focused($focused)
                .onSubmit { if usable { confirm() } }
            if let problem = folderNameProblem(verdict) {
                Text(problem).font(.caption).foregroundStyle(.red)
            }
            HStack {
                Spacer()
                Button(L10n.action_cancel(), role: .cancel) { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button(request.confirm) { confirm() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(!usable)
            }
        }
        .padding(20)
        .frame(minWidth: 320)
        .onAppear { focused = true }
        #if os(iOS)
        .presentationDetents([.medium])
        #endif
    }

    private func confirm() {
        submit(name)
        dismiss()
    }
}

/// Move to…: Top level, then every folder that takes one, by path (rule 24).
struct FolderMoveSheet: View {
    let target: FolderTarget
    let destinations: [MoveDestination]
    let move: (String?) -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List(destinations) { destination in
                Button {
                    move(destination.parent)
                    dismiss()
                } label: {
                    Label(
                        destination.label,
                        systemImage: destination.parent == nil ? "tray.2" : "folder"
                    )
                }
            }
            .navigationTitle(
                L10n.folder_move_title(name: folderLabel(role: target.folder.role, name: target.folder.name))
            )
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L10n.action_cancel()) { dismiss() }
                }
            }
        }
        .frame(minWidth: 340, minHeight: 360)
    }
}

extension ContentView {
    /// A folder change the server refused, until the user closes it (rule 28).
    @ViewBuilder var folderNoticeBanner: some View {
        if let notice = model.folderNotice {
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                Text(folderNoticeText(notice))
                    .font(.callout).fixedSize(horizontal: false, vertical: true)
                Spacer()
                Button(L10n.action_close()) { model.dismissFolderNotice() }
                    .buttonStyle(.borderless)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 9)
            .background(.thinMaterial)
            .overlay(alignment: .bottom) { Divider() }
        }
    }

    /// What a message row carries when dragged: the whole selection when it is part of one.
    func dragPayload(for row: SnapshotRow) -> PaneDragPayload {
        let keys = selection.contains(row) ? selection.keys : [SelectionKey(row)]
        return .messages(rows: keys.map(DraggedRow.init))
    }
}
