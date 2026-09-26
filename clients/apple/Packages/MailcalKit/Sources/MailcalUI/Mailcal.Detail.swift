import MailcalBindings
import SwiftUI

extension ContentView {
    /// The reading pane (third column): the open message inline, or the empty-state placeholder
    /// when nothing is selected. Selection drives it, tapping a row sets `openedMessage` and
    /// asks the core to fetch + sanitise the body (`open(_:)`).
    ///
    /// Several rows picked at once put the count here instead, over whatever the pane held, which
    /// is where the desktop states it (`docs/list-selection.md`, rule 10). Over, not instead of:
    /// swapping the subtree out would tear down the reading view and re-fetch the body the moment
    /// the selection dropped back to one row.
    @ViewBuilder var readingPane: some View {
        ZStack {
            if let opened = openedMessage {
                readingView(for: opened)
            } else {
                ReadingPanePlaceholder()
            }
            #if os(macOS)
            if let label = selection.paneLabel(mode: model.mode) {
                SelectionCountPane(label: label)
            }
            #endif
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
            onReply: { beginReply(opened.account, opened.key, subject: opened.subject, all: false) },
            onReplyAll: { beginReply(opened.account, opened.key, subject: opened.subject, all: true) },
            onForward: { beginForward(opened.account, opened.key, subject: opened.subject) },
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
            Divider()
            if model.showingOutbox {
                // The Outbox is not mail: it has its own rows, and none of the selection,
                // threading or infinite-scroll behaviour below applies to a list of things
                // that have not been sent.
                outboxList
            } else {
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
                // Over the rows, so the list keeps its size and the surfaces around it (the
                // header, the selection bar, the footer) do not move as a folder fills.
                .overlay {
                    EmptyMailboxView(reason: model.emptyReason) { settingsCategory = .accounts }
                }
            )
            }
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
            // A status line, and nothing else. New Mail moved to the window toolbar and Sync to
            // the actions bar, which is where a user coming from Outlook reaches for either; a
            // footer is the one place on the window neither of them is looked for.
            HStack {
                Text(footer).font(.caption).foregroundStyle(.secondary)
                // What the core has to say about mail arriving, or not: a pass nobody started,
                // or an account whose server asked us to wait. Named in the status line the
                // footer already draws rather than in a bar, so it takes no row of its own and
                // an account catching up in the background never moves the list.
                if let status = syncStatusText(model.syncProgress) {
                    syncStatusLabel(status)
                }
                Spacer()
            }
            .padding(10)
        }
    }

    /// What the list is showing, and nothing else.
    ///
    /// The search field left this row for the window toolbar (Mailcal.Toolbar.swift): it searches
    /// every account and folder rather than the column it sat over, so the window's centre is
    /// where it belongs. Settings is reached from the sidebar (or ⌘,), which is where every other
    /// destination in this app lives.
    private var messageListHeader: some View {
        Text(searchText.isEmpty ? currentFolderName : L10n.search_results())
            .font(.headline)
            .lineLimit(1)
            .truncationMode(.tail)
            .frame(minWidth: 72, maxWidth: .infinity, alignment: .leading)
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

    /// The status line's one caption, and the sentence behind it where it is a short label
    /// standing in for one. `nil` whenever there is nothing to say, which is almost always.
    ///
    /// **A pause takes the line ahead of the hint.** A pass that is downloading is already
    /// evident from the list filling; a pass that is waiting is evident from nothing at all,
    /// which is the question it answers. The core keeps the two sets disjoint, so this only has
    /// to order them.
    func syncStatusText(_ progress: SyncProgressSnapshot?) -> (label: String, detail: String?)? {
        if let paused = syncPauseText(progress) {
            return paused
        }
        guard let hint = syncHintText(progress) else { return nil }
        return (label: hint, detail: nil)
    }

    /// The caption itself. On a Mac the paused notice is a short label with the whole sentence
    /// behind a hover; everywhere else there is no hover to put it behind, so the line says the
    /// sentence. Either way the sentence is what assistive technology reads, on every platform.
    @ViewBuilder func syncStatusLabel(_ status: (label: String, detail: String?)) -> some View {
        #if os(macOS)
        Text(status.label)
            .font(.caption)
            .foregroundStyle(.secondary)
            .help(status.detail ?? status.label)
            .accessibilityLabel(status.detail ?? status.label)
        #else
        Text(status.detail ?? status.label)
            .font(.caption)
            .foregroundStyle(.secondary)
            .lineLimit(2)
        #endif
    }

    /// The paused notice: a server answered promptly and asked to be left alone for a while.
    /// Not an outage; the account keeps its mail, its credential and its badge.
    ///
    /// The wait is stated as an approximation. It was rounded up before it got here, and nothing
    /// re-reads the clock while the line is up.
    func syncPauseText(_ progress: SyncProgressSnapshot?) -> (label: String, detail: String?)? {
        guard let paused = progress?.throttled, let only = paused.first else { return nil }
        // Several at once are not named, exactly as with the hint: a status line cannot name
        // them all, and their waits have no shared end to state.
        guard paused.count == 1 else {
            return (L10n.sync_paused(), L10n.sync_paused_detail_accounts(count: paused.count))
        }
        let name = accountName(only.accountId)
        guard let minutes = only.resumesInMinutes else {
            // Two refusals in three name no instant. "Shortly" is the honest answer; a figure
            // here would be one we made up.
            return (L10n.sync_paused(), L10n.sync_paused_detail_soon(account: name))
        }
        return (
            L10n.sync_paused(),
            L10n.sync_paused_detail(account: name, count: Int(minutes))
        )
    }

    /// Names an account from the app's own account list, which is where every other surface gets
    /// the address; the id is the fallback for one removed mid-pass.
    func accountName(_ id: String) -> String {
        model.accounts.first { $0.id == id }?.email ?? id
    }

    /// The background-sync hint: which accounts are pulling mail down right now, and how far
    /// through their folders they are. `nil` whenever nothing is arriving unasked, which is
    /// almost always, the core admits an account only once its background pass has actually
    /// committed mail, so a poll that finds nothing renders nothing.
    func syncHintText(_ progress: SyncProgressSnapshot?) -> String? {
        guard let accounts = progress?.accounts, !accounts.isEmpty else { return nil }
        // Several at once carry no counts: one account in its folders and another in its bodies
        // have no shared unit to add up, and a status line cannot name them all anyway.
        guard let only = accounts.first, accounts.count == 1 else {
            return L10n.sync_hint_accounts(count: accounts.count)
        }
        let name = accountName(only.accountId)
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
