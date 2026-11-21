import Foundation
import MailcalBindings

/// How long the field stays quiet before the core is asked, matching Windows, Android and Linux
/// so a search costs the same on every platform.
private let searchDebounceMilliseconds = 250

extension MailboxModel {
    /// Set the active display time zone (an IANA id) via the selector.
    func setTimeZone(_ id: String) { app?.dispatch(intent: .setTimeZone(id: id)) }

    /// Adopt the device's reported time zone (the time zone notice's Update).
    func acceptTimeZoneChange() { app?.dispatch(intent: .acceptTimeZoneChange) }

    /// Keep the current zone (the time zone notice's Keep current).
    func dismissTimeZoneChange() { app?.dispatch(intent: .dismissTimeZoneChange) }

    func refresh() { app?.dispatch(intent: .refreshMail) }

    /// Files the Sent copy of a message that went out without one. Sends nothing.
    func retryUnfiledCopy() { app?.dispatch(intent: .retryUnfiledCopy) }

    /// Accepts the missing Sent copy, which answers its notice.
    func dismissUnfiledCopy() { app?.dispatch(intent: .dismissUnfiledCopy) }

    /// Sets the name one account's outgoing mail is sent under; empty clears it. Setup's name step
    /// asks this; an account's page asks the core through its own change.
    ///
    /// Passed through unchanged: the core sanitises it, and a second opinion here about what a
    /// `From` header may contain would only drift from the first (docs/sending.md).
    func setAccountSenderName(_ account: String, _ name: String) {
        app?.setAccountSenderName(account: account, name: name)
    }

    /// Switch the list between flat and threaded, a dispatched intent, so the Rust app
    /// re-projects and re-notifies (state stays in Rust).
    func setMode(_ mode: ViewMode) { app?.dispatch(intent: .setViewMode(mode: mode)) }

    /// Run a ranked full-text search, or clear it (empty query) to return to the folder
    /// view. The engine ranks; the app maps the hits back to messages and re-notifies.
    ///
    /// **Typing does not mean searching** (`docs/search.md`). A search is a full-text query per
    /// account plus a store read per hit, so dispatching one per keystroke runs a word's worth
    /// of them at once, each slowing the others; the core drops the ones the next keystroke
    /// superseded, but they are still work nobody wanted. The core is asked once, when the
    /// typing stops.
    ///
    /// Clearing stays immediate: leaving search is a navigation, and waiting a quarter of a
    /// second to give the user their mailbox back would read as the app hesitating.
    func search(_ query: String) {
        searchDebounce?.cancel()
        searchDebounce = nil
        guard !query.isEmpty else {
            app?.dispatch(intent: .search(query: nil))
            return
        }
        searchDebounce = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(searchDebounceMilliseconds))
            guard !Task.isCancelled else { return }
            self?.app?.dispatch(intent: .search(query: query))
        }
    }

    /// Open a message (by key) for reading: the app fetches + caches its source, extracts
    /// and sanitises the body, then signals `Surface::Reading` and `reading` updates. The
    /// stale body is cleared first so the reading view shows its loading state until this
    /// message's snapshot arrives, including on a retry of the same key after a load error.
    func openMessage(_ account: String, _ key: String) {
        reading = nil
        app?.dispatch(intent: .openMessage(account: account, key: key))
    }

    /// Resets the local database, once the person confirmed it. `advancedPage.reset` runs from
    /// the core's first signal until mail is downloaded again, and its outcome follows.
    func resetDatabase() { app?.dispatch(intent: .resetDatabase) }

    /// Focus one account's folders (by id), or the unified "all inboxes" view (`nil`). The
    /// core resets the selected folder when the account changes.
    func selectAccount(_ id: String?) {
        destination = .mail
        app?.dispatch(intent: .selectAccount(account: id))
    }

    /// Opens or shuts one account's folder tree in the sidebar; the core persists it.
    ///
    /// Deliberately does none of what ``selectAccount(_:)`` does, no destination change, no
    /// selection move. Expanding is not navigating, which is what lets several accounts stand
    /// open at once and keeps the tree as the user left it across a visit to the calendar
    /// (`docs/folder-pane.md`).
    func setAccountExpanded(_ id: String, _ expanded: Bool) {
        app?.dispatch(intent: .setAccountExpanded(account: id, expanded: expanded))
    }

    /// Opens or shuts the All Accounts group's tree. Not navigation: it moves neither the
    /// selected account nor the selected folder (`docs/folder-pane.md`).
    func setUnifiedExpanded(_ expanded: Bool) {
        app?.dispatch(intent: .setUnifiedExpanded(expanded: expanded))
    }

    /// Opens or shuts the folders filed inside one folder. The account travels with the key,
    /// as it does for ``selectFolder(in:key:)``: a folder key is unique only within its
    /// account. Not navigation either, so this does **not** open the folder's mail.
    func setFolderExpanded(_ account: String, _ key: String, _ expanded: Bool) {
        app?.dispatch(
            intent: .setFolderExpanded(account: account, key: key, expanded: expanded)
        )
    }

    /// One account's folders, as rows for the sidebar tree, each carrying an identity unique
    /// across the **whole** pane rather than within its own account.
    ///
    /// Empty for an account the snapshot has no folders for yet (a first sync still running).
    func folderRows(for account: String) -> [SidebarFolder] {
        let folders = accountFolders.first { $0.accountId == account }?.folders ?? []
        // A row inside a folder the user shut is not drawn. The core has already walked the
        // chain of parents, so this is a flag rather than a climb back up it here.
        return folders.filter(\.visible).map { SidebarFolder(account: account, folder: $0) }
    }

    /// The address of the account `id`, for display (`docs/accounts.md` rule 8). The snapshot
    /// names every account, mail or not; the id is the fallback for one removed mid-pass.
    func accountEmail(_ id: String) -> String {
        accounts.first { $0.id == id }?.email ?? accountEntry(id)?.address ?? id
    }

    /// Removes account `id`: the core drops it from the running app (rebuilding the switcher and
    /// list without it, falling back to the unified inbox if it was selected), clears every link
    /// the other accounts held to it, and erases its Keychain entry through the port it wrote it
    /// through. Dispatched: it answers at once, and the account's page says it runs until the
    /// account is gone. An erase that failed is said as `removalFailure`, and the last account
    /// gone returns the window to first-run setup (`docs/accounts.md` rule 11).
    func removeAccount(_ id: String) {
        changeAccount(.remove(account: id))
    }

    /// Focus one folder of one account, in the mail view.
    ///
    /// Both halves together: a folder key is unique only within its account, so the core takes no
    /// folder without one (`docs/folder-pane.md`, rule 14). For an account's whole mailbox use
    /// `selectAccount`.
    func selectFolder(account: String, folder: String) {
        destination = .mail
        app?.dispatch(intent: .selectFolder(account: account, key: folder))
    }

    /// Whether more rows can be shown than are currently loaded, the list checks this as its
    /// last row appears, before asking for the next page.
    var hasMore: Bool { UInt64(rows.count) < total }

    /// The folder's full row count (the footer total), `rows` holds only the visible window.
    var rowCount: Int { Int(total) }

    /// Grow the visible window by one page, the list calls this as its last row appears.
    /// Guarded so the burst of appearances issues one request, and it no-ops once every row
    /// is shown. The core grows the window and re-projects; the reconcile appends the new tail.
    func showMore() {
        guard !loadMorePending, hasMore else { return }
        loadMorePending = true
        app?.dispatch(intent: .showMore)
    }

    /// Switch to the calendar agenda and sync it.
    func showCalendar() {
        destination = .calendar
        app?.dispatch(intent: .refreshCalendar)
    }

    /// Send a plain-text message; the app files it via SMTP, then re-syncs.
    func submit(to: String, subject: String, body: String) {
        app?.dispatch(intent: .submitMail(to: to, subject: subject, body: body))
    }

    /// The persisted reply/forward quoting settings, the style seeds a reply's composer, and
    /// `perMessage` decides whether that composer offers a style picker at all. Reads the app,
    /// falling back to the published mirror before it is ready.
    func quoteSettingsNow() -> QuoteSettings {
        app?.quoteSettings() ?? quoteSettings
    }

    /// Sends one preference change. It takes effect in the core before this returns and is saved
    /// behind it; what the model holds of it follows on the `Surface::Settings` signal, and a save
    /// that fails is the core's notice, not a value kept here.
    func preference(_ intent: PreferenceIntent) {
        app?.dispatch(intent: .preferences(intent: intent))
    }

    /// Sets the default reply/forward quote style.
    func setQuoteStyle(_ style: QuoteStyleKind) {
        preference(.setQuoteStyle(style: style))
    }

    /// Records the usage-statistics decision, and it *is* a decision either way: passing `false`
    /// stores a decline, which is what stops us asking again. Opting in mints the install id;
    /// opting out clears it and asks the backend to erase what it holds (GDPR Art. 17).
    ///
    /// `analyticsConsent` follows on the `Surface::Settings` signal the decision raises.
    func setAnalyticsConsent(_ enabled: Bool) {
        preference(.setAnalyticsConsent(enabled: enabled))
    }

    /// The literal JSON the core would put on the wire, for the "see exactly what we send" panel.
    func analyticsPayloadPreview() -> String {
        app?.analyticsPayloadPreview() ?? ""
    }

    /// Sets whether the composer offers a per-message quote-style override; without one, a reply
    /// uses the default style above and shows no picker.
    func setQuoteStylePerMessage(_ perMessage: Bool) {
        preference(.setQuoteStylePerMessage(perMessage: perMessage))
    }

    /// Sets the app-level default send account, the one Settings → Composing offers.
    func setDefaultSendAccount(_ id: String) {
        preference(.setDefaultSendAccount(account: id))
    }

    /// Sets and persists the diagnostics "include more detail" choice: ON logs the core's
    /// DEBUG detail for a support session, OFF returns to the contract's INFO default
    /// (docs/logging.md). Applied to the live core at once; every core-construction site boots
    /// from the same persisted pref (`DiagnosticsPrefs.coreLogLevel`), so the choice survives
    /// a relaunch and reaches the iOS background worker too.
    func setDiagnosticsDebugLogging(_ enabled: Bool) {
        DiagnosticsPrefs.debugLogging = enabled
        app?.setLogLevel(level: DiagnosticsPrefs.coreLogLevel)
    }

    /// Sets what one swipe direction does. The two directions are independent.
    func setSwipeAction(_ direction: SwipeDirection, _ action: SwipeActionKind) {
        preference(.setSwipeAction(direction: direction, action: action))
    }

    /// Mark a message read or unread (by key, on its owning `account`), then re-sync.
    func markRead(_ account: String, _ key: String, _ read: Bool) {
        app?.dispatch(intent: .markRead(account: account, key: key, read: read))
    }

    /// Flag or unflag a message (by key, on its owning `account`), then re-sync.
    func setFlagged(_ account: String, _ key: String, _ flagged: Bool) {
        app?.dispatch(intent: .setFlagged(account: account, key: key, flagged: flagged))
    }

    /// Delete a message, move it to Trash, recoverable (by key, on its owning
    /// `account`), then re-sync.
    func delete(_ account: String, _ key: String) {
        app?.dispatch(intent: .delete(account: account, key: key))
    }

    /// Archive a message, move it to the account's Archive folder (by key, on its owning
    /// `account`), then re-sync.
    func archive(_ account: String, _ key: String) {
        app?.dispatch(intent: .archive(account: account, key: key))
    }

    /// Report a message as junk, which files it under Junk (`docs/reporting.md`).
    func markAsSpam(_ account: String, _ key: String) {
        app?.dispatch(intent: .markAsSpam(account: account, key: key))
    }

    /// Report a message as not junk, which files it back in the Inbox.
    func markAsNotSpam(_ account: String, _ key: String) {
        app?.dispatch(intent: .markAsNotSpam(account: account, key: key))
    }

    /// The menu over `rows`, as the core decides it (`docs/row-actions.md`). Read from the
    /// published snapshot without waiting, so it is asked for as the menu opens.
    func rowMenu(_ rows: [SelectedRow]) -> RowMenu {
        app?.rowMenu(rows: rows) ?? RowMenu(sections: [])
    }

    /// The mail command bar over `rows`, as the core decides it (`docs/command-bar.md`): read off
    /// the same menu a right-click over them opens, from the published snapshot without waiting.
    func commandBar(_ rows: [SelectedRow]) -> CommandBar {
        app?.commandBar(rows: rows) ?? CommandBar(sections: [], menu: [])
    }

    /// The message the reading pane opens once `account`'s `key` leaves the list, as the core
    /// decides it (`docs/reading-actions.md`, "Where the pane goes next"). `expanded` names the
    /// conversations the list shows expanded.
    func stopAfterRemoving(_ account: String, _ key: String, expanded: [SelectedRow]) -> MenuMessage? {
        app?.stopAfterRemoving(account: account, key: key, expanded: expanded)
    }

    /// Permanently delete a message, irreversible (by key, on its owning `account`),
    /// then re-sync.
    func permanentlyDelete(_ account: String, _ key: String) {
        app?.dispatch(intent: .permanentlyDelete(account: account, key: key))
    }

    /// Run one action over **every** selected row, as a single batch: one optimistic hide and one
    /// re-sync per account, rather than one of each per row (`docs/list-selection.md`, rule 8).
    /// A conversation row stands for its whole thread, which the core expands itself.
    func actOnSelection(_ rows: [SelectedRow], _ action: BulkAction) {
        guard !rows.isEmpty else { return }
        app?.dispatch(intent: .actOnSelection(rows: rows, action: action))
    }

    // MARK: Agent (MCP) access, docs/mcp.md

    /// One change to what assistants may do. The core holds it before `dispatch` returns, so the
    /// page read straight after draws it and a switch does not spring back until the signal.
    func mcp(_ intent: McpIntent) {
        guard let app else { return }
        app.dispatch(intent: .mcp(intent: intent))
        advancedPage = app.advancedPage()
    }
}
