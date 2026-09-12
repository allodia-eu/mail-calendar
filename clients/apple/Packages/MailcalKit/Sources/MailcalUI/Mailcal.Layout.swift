import MailcalBindings
import SwiftUI

extension ContentView {
    /// The platform-adaptive base. macOS keeps its 3-pane split (sidebar | list | reading via
    /// HSplitView). iPad uses a 3-column NavigationSplitView (all columns visible at regular
    /// width, so the action-driven sidebar works without explicit navigation). iPhone uses a
    /// NavigationStack, the list with a leading menu for accounts/folders, pushing the reading
    /// view when a message opens. The shared banners/sheets/dialogs in `mainView` sit on top.
    @ViewBuilder var baseLayout: some View {
        #if os(macOS)
        macOSLayout
        #else
        if hSize == .compact { iPhoneLayout } else { iPadLayout }
        #endif
    }

    #if os(macOS)
    /// macOS keeps the desktop shell as a direct split view. `NavigationSplitView` can collapse
    /// or overlay columns on macOS in ways that are useful for document navigation but wrong for
    /// this mail layout, where sidebar, list, and reading pane must all remain visible.
    ///
    /// **The split's two panes must survive a destination change**, that is what keeps the widths
    /// you drag. AppKit restores an autosaved layout when a split view is *set up*
    /// and never again, so a split whose panes are torn down under it loses the divider and then
    /// saves the collapsed result over the good one. Switching Mail → Calendar → Mail used to take
    /// the sidebar to its `maxWidth` and the list to its `minWidth` for exactly that reason.
    ///
    /// Two things together fix it, and each was necessary:
    ///
    /// 1. Destinations that show more than one pane put theirs in a **nested** split
    ///    (`destinationPanes`), so this split's pane *count* no longer changes with the
    ///    destination, and each nested split is genuinely created when you enter its
    ///    destination, which is the moment AppKit restores.
    /// 2. The second pane is wrapped in a container that outlives the branch change. A `switch`
    ///    written directly here is a `_ConditionalContent`, and SwiftUI rebuilds that subtree
    ///    whenever the branch changes, **even between two branches of the same shape** (Mail →
    ///    Contacts → Mail, both nested splits, still snapped the sidebar to 320). The `ZStack`
    ///    persists, so the swap happens *inside* a pane the split never sees change.
    var macOSLayout: some View {
        HSplitView {
            sidebarList(showsCalendarAndContacts: true)
                .frame(minWidth: 220, idealWidth: 240, maxWidth: 320)
                .background(SplitViewAutosave(name: AppPrefs.autosaveName("AllodiaMailMacSidebarV3")))

            ZStack { destinationPanes }
        }
        .frame(minWidth: 1_080, minHeight: 520)
    }

    /// Everything to the right of the sidebar. Each destination owns its own split and its own
    /// autosave name, so the widths a user sets in the mailbox and in contacts are separate
    /// choices that both survive leaving and coming back, see `macOSLayout` for why the nesting
    /// is load-bearing.
    @ViewBuilder private var destinationPanes: some View {
        switch model.destination {
        case .calendar:
            // One pane, the grid takes the whole width beside the sidebar, so there is no
            // divider here to remember.
            calendarDetail
                .frame(minWidth: 620)
        case .contacts:
            // List | detail, like mail, a list of people is something you pick one of, where
            // the calendar is one wide grid with nothing to select beside it.
            HSplitView {
                contactsList
                    .frame(minWidth: 300, idealWidth: 360, maxWidth: 480)
                    .background(
                        SplitViewAutosave(name: AppPrefs.autosaveName("AllodiaMailMacContactsV3"))
                    )
                contactsDetailPane
                    .frame(minWidth: 420, idealWidth: 620)
            }
        case .mail:
            // The actions bar spans the split rather than sitting inside the list column: it acts
            // on rows, but its buttons are as wide as the words on them, and a column the user can
            // drag to 420 points made the last of them something to scroll sideways for.
            VStack(spacing: 0) {
                selectionBar
                Divider()
                mailPanes
            }
        }
    }

    /// The mailbox's two panes, under the actions bar that spans them.
    private var mailPanes: some View {
        HSplitView {
            messageList
                .frame(minWidth: 420, idealWidth: 540, maxWidth: 720)
                .background(SplitViewAutosave(name: AppPrefs.autosaveName("AllodiaMailMacMailV3")))
            // The second column is the reading pane, or, while a draft is open, the composer in
            // its place. Writing a message no longer blacks out the mailbox behind a sheet: the
            // sidebar and the list stay live, and clicking another message asks before it drops
            // the draft (openGuardingDraft). Swapping the two keeps this split at two panes, so
            // opening a draft doesn't disturb the divider either.
            detailColumn
                .frame(minWidth: 420, idealWidth: 760)
        }
    }

    /// The macOS detail column: the composer when one is open, else the reading pane.
    @ViewBuilder var detailColumn: some View {
        if let compose {
            composeContent(compose)
        } else {
            readingPane
        }
    }
    #endif

    #if os(iOS)
    /// The touch message list: native chrome, a `.searchable` bar, a `.plain` list that pulls down
    /// to sync, and the horizon and sync strips as insets around the rows.
    ///
    /// **The list is this view, not a row inside a stack.** The navigation bar's title and search
    /// field size themselves against the scroll view the navigation stack's content *is*, and
    /// wrapped in a `VStack` it was not that: the field became pinned chrome that hid itself the
    /// moment the refresh control moved the offset, so a pull dropped the field and threw the rows
    /// up by its height. The two strips are safe-area insets for the same reason: they draw where
    /// stack rows drew them without standing between the list and the bar.
    var compactMessageList: some View {
        selectionBehaviour(
            List {
                let rows = visibleRows
                ForEach(rows, id: \.rowID) { row in
                    rowView(row)
                        .listRowBackground(rowHighlight(row))
                        .onAppear {
                            if row.rowID == rows.last?.rowID {
                                Task { @MainActor in model.showMore() }
                            }
                        }
                }
            }
            .listStyle(.plain)
            // Sync, on a platform with no room for a button that says so.
            .refreshable { await pullToSync() }
            .searchable(text: $searchText, prompt: Text(L10n.search_placeholder()))
            .onChange(of: searchText) { _, query in model.search(query) }
            // Under the search field, over the rows: how far back a search looked, then the count
            // and the batched actions for the rows they describe. Neither is up most of the time,
            // and an inset of nothing is no inset, so the list keeps its full height.
            .safeAreaInset(edge: .top, spacing: 0) {
                VStack(spacing: 0) {
                    SearchHorizonStrip(horizon: model.searchHorizon) { settingsCategory = .accounts }
                    selectionBar
                }
            }
            .safeAreaInset(edge: .bottom, spacing: 0) { VStack(spacing: 0) { syncStrip } }
        )
    }

    /// What the list says about mail arriving, below the rows and outside the pull-to-refresh box
    /// (`docs/sync-progress.md`). As the list's first *row* it pushed every message down when a
    /// background sync began and back up when it ended.
    ///
    /// The phone has no footer to put the background hint in, so it shares this strip: the same
    /// edge, without the bar. Both can be true at once (an awaited download while a poll tick
    /// catches another account up); the bar is the one the user is waiting on, so it wins.
    @ViewBuilder private var syncStrip: some View {
        if let progress = model.syncProgress, progress.active {
            Divider()
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(syncProgressText(progress)).font(.caption).foregroundStyle(.secondary)
                Spacer()
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
        } else if let hint = syncHintText(model.syncProgress) {
            Divider()
            HStack {
                Text(hint).font(.caption).foregroundStyle(.secondary)
                Spacer()
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
        }
    }

    /// Pull down to sync: the gesture that carries a touch platform's whole sync affordance.
    ///
    /// `refresh()` is fire-and-forget, so what holds the spinner up is the core's own progress:
    /// a short hold first, so a pass that has not raised itself yet is not read as one that
    /// never started, then the length of whatever it turns out to be. A refresh that starts no
    /// sync at all (offline, or nothing new on the server) clears after the hold alone. The same
    /// shape Android's `PullToRefreshBox` uses, so the two phones spin for the same duration.
    func pullToSync() async {
        model.refresh()
        try? await Task.sleep(for: .milliseconds(700))
        while model.syncProgress?.active == true {
            try? await Task.sleep(for: .milliseconds(200))
        }
    }

    #endif

    /// The current scope's title. `listTitle` decides it, so the rule can be asserted without
    /// a window; see it in Mailcal.Sidebar.swift.
    var currentFolderName: String {
        listTitle(
            selectedAccount: model.selectedAccount,
            selectedFolder: model.selected,
            folders: model.folders
        )
    }
}
