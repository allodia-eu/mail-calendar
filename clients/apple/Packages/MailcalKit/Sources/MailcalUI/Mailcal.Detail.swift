import MailcalBindings
import SwiftUI

extension ContentView {
    /// The reading pane (third column): the open message inline, or the empty-state placeholder
    /// when nothing is selected. Selection drives it, tapping a row sets `openedMessage` and
    /// asks the core to fetch + sanitise the body (`open(_:)`).
    @ViewBuilder var readingPane: some View {
        if let opened = openedMessage {
            readingView(for: opened)
        } else {
            ReadingPanePlaceholder()
        }
    }

    /// The reading view for an opened message, shared by the macOS/iPad reading pane and the
    /// iPhone push destination.
    ///
    /// Archive/delete move the message out of the folder, so the pane cannot keep showing it. Where
    /// there is a pane beside the list it advances to the next message down (Mailcal.AutoAdvance.swift)
    /// so a mailbox can be worked through without returning to the list for each one; on the iPhone,
    /// where reading is a pushed screen, clearing still pops back to the list.
    func readingView(for opened: OpenedMessage) -> some View {
        ReadingView(
            model: model,
            message: opened,
            onReply: { beginReply(opened.account, opened.key, all: false) },
            onReplyAll: { beginReply(opened.account, opened.key, all: true) },
            onForward: { beginForward(opened.account, opened.key) },
            onArchive: {
                // Chosen before the dispatch, while the row it is relative to is still on screen.
                let next = stopAfterRemoving(opened)
                model.archive(opened.account, opened.key)
                settleReadingPane(next)
            },
            onDelete: {
                let next = stopAfterRemoving(opened)
                model.delete(opened.account, opened.key)
                settleReadingPane(next)
            }
        )
    }

    /// Whether `row` is the open message, to highlight it. A threaded row highlights per
    /// sub-row inside its own cell (`threadRow`), so it never highlights the whole cell here.
    func isReadingRow(_ row: SnapshotRow) -> Bool {
        guard let opened = openedMessage else { return false }
        switch row {
        case .flat(let message):
            return message.account == opened.account && message.key == opened.key
        case .thread:
            return false
        }
    }

    /// The background a list row is drawn on: selected, open in the reading pane, or neither.
    func rowHighlight(_ row: SnapshotRow) -> Color {
        if isSelectedRow(row) { return Color.accentColor.opacity(0.25) }
        return isReadingRow(row) ? Color.accentColor.opacity(0.15) : Color.clear
    }

    var messageList: some View {
        VStack(spacing: 0) {
            messageListHeader
            // How far back the results reach, between the header and the rows. It appears and
            // disappears only as search itself begins and ends, which already replaces the whole
            // list, so unlike the progress bar below, it moves no rows the user is reading.
            SearchHorizonStrip(horizon: model.searchHorizon) { settingsCategory = .accounts }
            // The count and the batched actions, over the rows they describe. Nothing selected
            // draws nothing, so the list keeps its full height the rest of the time.
            selectionBar
            Divider()
            selectionBehaviour(
                List {
                    let rows = visibleRows
                    ForEach(rows, id: \.rowID) { row in
                        rowView(row)
                            // Highlight the selected rows, and the one whose message is open in
                            // the reading pane. Selection wins where they disagree: it is what
                            // the bar's buttons would act on, so it is what has to be legible.
                            .listRowBackground(rowHighlight(row))
                            // Infinite scroll: when the last row appears, ask the core for the
                            // next page. It grows the window and re-projects; the model coalesces
                            // the burst and no-ops once every row is shown.
                            .onAppear {
                                // Defer to the next main-actor hop so we don't grow the list from
                                // inside the table's layout pass (the source of the NSTableView
                                // "reentrant operation" warning).
                                if row.rowID == rows.last?.rowID {
                                    Task { @MainActor in model.showMore() }
                                }
                            }
                    }
                }
            )
            Divider()
            // Background-download progress: a thin bar with a "downloading Y of X" count, shown
            // only while a sync is fetching mail (the rows arrive on their own signal).
            //
            // **Below the list, never above it.** Above, every appearance and disappearance
            // resized the list and moved every row under the pointer, for a background pass the
            // user did not start. Here it grows into the footer's own edge instead.
            if let progress = model.syncProgress, progress.active {
                HStack(spacing: 8) {
                    if let total = progress.total, total > 0 {
                        ProgressView(value: Double(progress.fetched), total: Double(total))
                    } else {
                        ProgressView().progressViewStyle(.linear)
                    }
                    Text(syncProgressText(progress)).font(.caption).foregroundStyle(.secondary)
                }
                .padding(.horizontal, 12).padding(.vertical, 6)
                Divider()
            }
            HStack {
                Text(footer).font(.caption).foregroundStyle(.secondary)
                // The background-sync hint: a pass nobody started, named in the status line the
                // footer already draws rather than in a bar. It takes no row of its own, so an
                // account catching up in the background never moves the list.
                if let hint = syncHintText(model.syncProgress) {
                    Text(hint).font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                Button(L10n.action_compose()) { compose = .new }
                    .buttonStyle(.borderedProminent)
                Button(L10n.action_refresh()) { model.refresh() }
            }
            .padding(10)
        }
    }

    private var messageListHeader: some View {
        HStack(spacing: 10) {
            Text(searchText.isEmpty ? currentFolderName : L10n.search_results())
                .font(.headline)
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(minWidth: 72, maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 4) {
                Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                TextField(L10n.search_placeholder(), text: $searchText)
                    .textFieldStyle(.roundedBorder)
                    .frame(minWidth: 140, idealWidth: 180, maxWidth: 220)
                    .onChange(of: searchText) { _, query in model.search(query) }
            }
            // Settings is reached from the sidebar (or ⌘,), which is where every other destination
            // in this app lives. A second gear over the message list offered the same screen twice
            // from two different places, so this header stays on what it is for: the folder and the
            // search field.
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    /// The calendar: the paged time grid, the month, or the agenda.
    ///
    /// The grid is a **pull with an argument**, not a pushed snapshot, see MailcalModel.Calendar.swift.
    var calendarDetail: some View {
        CalendarScreenView(model: model)
    }


    var footer: String {
        // The folder's full total, not the visible window (rows holds only the loaded page).
        switch model.mode {
        case .flat: return L10n.mailbox_count_messages(count: model.rowCount)
        case .threaded: return L10n.mailbox_count_conversations(count: model.rowCount)
        }
    }

    /// The background-sync hint: which accounts are pulling mail down right now, and how far
    /// through their folders they are. `nil` whenever nothing is arriving unasked, which is
    /// almost always, the core admits an account only once its background pass has actually
    /// committed mail, so a poll that finds nothing renders nothing.
    ///
    /// Named from the app's own account list, which is where every other surface gets the
    /// address; the id is a fallback for an account that has since been removed mid-pass.
    func syncHintText(_ progress: SyncProgressSnapshot?) -> String? {
        guard let accounts = progress?.accounts, !accounts.isEmpty else { return nil }
        // Several at once carry no counts: one account in its folders and another in its bodies
        // have no shared unit to add up, and a status line cannot name them all anyway.
        guard let only = accounts.first, accounts.count == 1 else {
            return L10n.sync_hint_accounts(count: accounts.count)
        }
        let name = model.accounts.first { $0.id == only.accountId }?.email ?? only.accountId
        if only.warmingBodies {
            return L10n.sync_hint_bodies(account: name, done: syncCount(UInt64(only.bodiesDone)))
        }
        return L10n.sync_hint_account(
            account: name,
            done: String(only.foldersDone),
            total: String(only.foldersTotal)
        )
    }

    /// The "downloading Y of X" caption beside the sync bar (thousands-separated, or an
    /// indeterminate variant until the total is known).
    func syncProgressText(_ progress: SyncProgressSnapshot) -> String {
        if let total = progress.total {
            return L10n.sync_downloading(fetched: syncCount(progress.fetched), total: syncCount(total))
        }
        return L10n.sync_downloading_indeterminate(fetched: syncCount(progress.fetched))
    }

    func syncCount(_ value: UInt64) -> String { value.formatted(.number) }
}
