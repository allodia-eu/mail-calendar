// The mail command bar (`docs/command-bar.md`), drawn as the core answers it: on the Mac, one row
// across the window under its title bar, New Mail over the sidebar and the actions over the list
// and the reading pane; on the iPhone and the iPad, the bar the selection mode raises. Which items
// there are, which respond and what Delete does are the core's; this file lays them out and runs
// what is picked through the row menu's own path.

import MailcalBindings
import SwiftUI

extension ContentView {
    /// The selection as the core names rows.
    var selectedRows: [SelectedRow] {
        selection.selectedRows(expanded: expandedConversations)
    }

    /// The core's bar over the selection, with Move responding only where this client has a
    /// folder to offer it (`canAskMessageMove`), exactly as the row menu's Move does.
    var currentCommandBar: CommandBar {
        var bar = model.commandBar(selectedRows)
        let movable = canAskMessageMove(Array(selection.keys))
        for section in bar.sections.indices {
            for at in bar.sections[section].items.indices
            where bar.sections[section].items[at].command == .act(action: .moveToFolder) {
                bar.sections[section].items[at].enabled =
                    bar.sections[section].items[at].enabled && movable
            }
        }
        return bar
    }

    /// Runs a bar item as the same item in a row menu over the selection runs.
    func runBarItem(_ item: CommandItem) {
        guard let menuItem = item.menuItem else {
            model.refresh()
            return
        }
        perform(menuItem, over: selection.keys)
    }

    /// The Delete key and Edit ▸ Delete: the bar's Delete over the selection, to Trash or, where
    /// the bar says so, for good after asking (`docs/list-selection.md`, rule 9).
    func deleteSelection() {
        guard let item = Self.deleteItem(in: model.commandBar(selectedRows)), item.enabled,
            let menuItem = item.menuItem
        else { return }
        perform(menuItem, over: selection.keys)
    }

    /// The reading pane's Delete for `opened`: the bar's Delete over that one message, so in
    /// Trash it deletes for good after asking rather than doing nothing (rule 8).
    func deleteOpened(_ opened: OpenedMessage) {
        let row = SelectedRow.message(account: opened.account, key: opened.key)
        let item = Self.deleteItem(in: model.commandBar([row]))
        let remove = {
            // Chosen before the dispatch, while the row it is relative to is still on screen.
            let next = stopAfterRemoving(opened)
            if item?.command == .act(action: .permanentlyDelete) {
                model.permanentlyDelete(opened.account, opened.key)
            } else {
                model.delete(opened.account, opened.key)
            }
            settleReadingPane(next)
        }
        if item?.confirm == true {
            guard item?.enabled == true else { return }
            pendingPurge = PurgeRequest(message: PurgeWording.message(for: [row]), run: remove)
        } else {
            remove()
        }
    }

    static func deleteItem(in bar: CommandBar) -> CommandItem? {
        bar.sections.lazy.flatMap(\.items).first {
            $0.command == .act(action: .delete) || $0.command == .act(action: .permanentlyDelete)
        }
    }

    #if os(macOS)
    /// The Mac's bar: New Mail in a slot as wide as the sidebar, so the actions begin at the
    /// message list's leading edge and follow the sidebar's divider, as Outlook's do. A later
    /// collapsed sidebar narrows the slot to New Mail's own width and the actions follow it.
    var mailCommandBar: some View {
        HStack(spacing: 0) {
            Button {
                beginNewMessage()
            } label: {
                Label(L10n.action_compose(), systemImage: "square.and.pencil")
                    .lineLimit(1)
            }
            .modifier(GlassButton())
            .controlSize(.large)
            .labelStyle(.titleAndIcon)
            .fixedSize()
            .help(L10n.action_compose())
            .padding(.leading, chromeLeading)
            .frame(width: sidebarWidth, alignment: .leading)
            CommandOverflowRow(bar: currentCommandBar, run: runBarItem)
                .padding(.horizontal, 12)
        }
        .padding(.vertical, 7)
        // The sidebar's own material, run up under the title row: the title row, the bar and the
        // sidebar are one grey frame of window chrome around the white panes, as in Outlook, and
        // it follows dark mode and the desktop's tint as the sidebar does. A colour cannot: the
        // window's background colour is white on current macOS.
        .background { SidebarMaterial().ignoresSafeArea(edges: .top) }
        .background(WindowButtonsLeading(leading: $chromeLeading))
    }
    #endif

    /// The phone's and the iPad's bar, raised with the selection mode: the core's actions as
    /// icons, a list column being too narrow for words, then the mode's own Select all and Done.
    /// Each button is still a `Label`, so its name is what assistive technology reads.
    @ViewBuilder
    var selectionBar: some View {
        #if os(iOS)
        if editMode.isEditing {
            HStack(spacing: 18) {
                ForEach(Array(currentCommandBar.sections.flatMap(\.items).enumerated()), id: \.offset) {
                    _, item in
                    Button(role: item.confirm ? .destructive : nil) { runBarItem(item) } label: {
                        Label(item.title, systemImage: item.systemImage)
                    }
                    .disabled(!item.enabled)
                }
                Spacer()
                Button { selection.selectAll(visibleRows) } label: {
                    Label(L10n.action_select_all(), systemImage: "checklist")
                }
                // ⌘A from a hardware keyboard, while the mode is on: SwiftUI's list answers the
                // system's Select All only on the Mac.
                .keyboardShortcut("a", modifiers: .command)
            }
            .labelStyle(.iconOnly)
            .buttonStyle(.borderless)
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .background(.quaternary.opacity(0.5))
        }
        #endif
    }
}

#if os(macOS)
/// A standalone button on the window's chrome: Liquid Glass where the system has it, as New Mail
/// had in the toolbar and as Outlook's has, and the bordered button it replaced before macOS 26.
/// Not the accent-filled style: on macOS that is the default action of a dialog, which a window's
/// standing chrome does not have.
private struct GlassButton: ViewModifier {
    func body(content: Content) -> some View {
        if #available(macOS 26, *) {
            content.buttonStyle(.glass)
        } else {
            content.buttonStyle(.bordered)
        }
    }
}

/// The actions of a `CommandBar` on one line, with as many on the bar as fit and the rest behind
/// ⋯ (`docs/command-bar.md`, rule 4). Every split is offered to `ViewThatFits`, most items first,
/// so the measuring is SwiftUI's own and follows the language and the text size.
struct CommandOverflowRow: View {
    let bar: CommandBar
    let run: (CommandItem) -> Void

    /// Every bar item in order, with the section it belongs to.
    private var items: [(section: Int, item: CommandItem)] {
        bar.sections.enumerated().flatMap { index, section in
            section.items.map { (index, $0) }
        }
    }

    var body: some View {
        let items = items
        ViewThatFits(in: .horizontal) {
            ForEach((0...items.count).reversed(), id: \.self) { shown in
                row(items, showing: shown)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func row(_ items: [(section: Int, item: CommandItem)], showing shown: Int) -> some View {
        HStack(spacing: 14) {
            ForEach(0..<shown, id: \.self) { at in
                if at > 0, items[at].section != items[at - 1].section {
                    Divider().frame(height: 18)
                }
                button(items[at].item)
            }
            let hidden = Array(items[shown...])
            if !hidden.isEmpty || !bar.menu.isEmpty {
                more(hidden)
            }
        }
        .labelStyle(.titleAndIcon)
        .buttonStyle(.borderless)
        .fixedSize()
    }

    private func button(_ item: CommandItem) -> some View {
        Button(role: item.confirm ? .destructive : nil) { run(item) } label: {
            Label(item.title, systemImage: item.systemImage).lineLimit(1)
        }
        .disabled(!item.enabled)
        .help(item.title)
    }

    /// The overflow: what left the bar, in bar order with its dividers, then what always stands
    /// here past a separator.
    private func more(_ hidden: [(section: Int, item: CommandItem)]) -> some View {
        Menu {
            ForEach(Array(hidden.enumerated()), id: \.offset) { at, entry in
                if at > 0, entry.section != hidden[at - 1].section { Divider() }
                button(entry.item)
            }
            if !hidden.isEmpty, !bar.menu.isEmpty { Divider() }
            ForEach(Array(bar.menu.enumerated()), id: \.offset) { _, item in button(item) }
        } label: {
            Image(systemName: "ellipsis")
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
        .accessibilityLabel(L10n.a11y_more_actions())
        .help(L10n.a11y_more_actions())
    }
}
#endif
