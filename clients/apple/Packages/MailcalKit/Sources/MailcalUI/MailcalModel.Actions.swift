import Foundation
import MailcalBindings

/// How long the field stays quiet before the core is asked, matching Windows, Android and Linux
/// so a search costs the same on every platform.
private let searchDebounceMilliseconds = 250

extension MailboxModel {
    /// Whether the device is currently offline, drives the offline banner.
    var isOffline: Bool { connectivity?.offline ?? false }

    /// Whether `accountId`'s server couldn't be reached on its last sync (while online), drives
    /// the per-account warning badge in the switcher.
    func isAccountUnreachable(_ accountId: String) -> Bool {
        connectivity?.unreachableAccounts.contains(accountId) ?? false
    }

    /// The friendly emails of Microsoft accounts whose calendar is withheld for lack of the
    /// calendar OAuth scope (connected before calendar support, or revoked consent), drives the
    /// "reconnect for calendar" banner. Mail is unaffected; re-authenticating grants the scope.
    var calendarReauthEmails: [String] {
        (connectivity?.calendarReauthAccounts ?? []).map(accountEmail)
    }

    /// The friendly emails of Microsoft accounts whose mail write/send is withheld for lack of the
    /// `Mail.ReadWrite` / `Mail.Send` OAuth scopes (connected before those scopes, or revoked
    /// consent), drives the "reconnect to send and manage mail" banner. Reading is unaffected;
    /// re-authenticating grants the full scope set, clearing this and any calendar prompt at once.
    var mailReauthEmails: [String] {
        (connectivity?.mailReauthAccounts ?? []).map(accountEmail)
    }

    /// The accounts whose stored sign-in the server has stopped accepting, an expired or revoked
    /// OAuth grant, or a password it now refuses. Nothing syncs until the user signs in again, and
    /// retrying never helps, so this drives its own banner rather than the unreachable badge (the
    /// core already keeps such an account out of `unreachableAccounts`). Each entry carries the
    /// provider so the button can launch the right sign-in, `nil` for an account whose family the
    /// core doesn't know, which the banner treats as "update it in Settings". The id rides along
    /// because a JMAP re-authentication is addressed to the *account*, not to an address: it
    /// re-authorises that account's own persisted grant.
    var signInExpiredAccounts: [(id: String, email: String, provider: AccountProvider?)] {
        (connectivity?.signinExpiredAccounts ?? []).map { id in
            (id: id, email: accountEmail(id), provider: app?.accountProvider(accountId: id))
        }
    }

    /// Set the active display time zone (an IANA id) via the selector.
    func setTimeZone(_ id: String) { app?.dispatch(intent: .setTimeZone(id: id)) }

    /// Adopt the device's reported time zone (the user accepted the change prompt).
    func acceptTimeZoneChange() { app?.dispatch(intent: .acceptTimeZoneChange) }

    /// Keep the current zone (the user dismissed the change prompt).
    func dismissTimeZoneChange() { app?.dispatch(intent: .dismissTimeZoneChange) }

    func refresh() { app?.dispatch(intent: .refreshMail) }

    /// Files the Sent copy of a message that went out without one. Sends nothing.
    func retryUnfiledCopy() { app?.dispatch(intent: .retryUnfiledCopy) }

    /// Accepts the missing Sent copy and closes the question.
    func dismissUnfiledCopy() { app?.dispatch(intent: .dismissUnfiledCopy) }

    /// Sets one account's **per-account** sync depth (a month count; `0` = all mail) and
    /// reconnects that account with the new window (widening fetches older mail, narrowing stops
    /// fetching it). The settings surface re-signals with the new state.
    func setAccountSyncDepth(_ account: String, _ months: UInt16) {
        app?.setAccountSyncDepth(account: account, months: months)
    }

    /// Sets the name one account's outgoing mail is sent under; empty clears it.
    ///
    /// Passed through unchanged: the core sanitises it, and a second opinion here about what a
    /// `From` header may contain would only drift from the first (docs/sending.md).
    func setAccountSenderName(_ account: String, _ name: String) {
        app?.setAccountSenderName(account: account, name: name)
    }

    /// Sets one account's **per-account** message-size cap (a megabyte count; `0` = no limit).
    /// Raising it downloads what the lower cap skipped; lowering it forgets the cached copies it
    /// may no longer keep, which needs no server. The mail itself is never removed either way:
    /// only its offline copy. The settings surface re-signals with the new state.
    func setAccountMessageSizeLimit(_ account: String, _ megabytes: UInt16) {
        app?.setAccountMessageSizeLimit(account: account, megabytes: megabytes)
    }

    /// Choose how an account receives new mail: push (IMAP IDLE, only when the server
    /// supports it) or interval polling. The core persists it and restarts the account's
    /// background sync; the settings surface re-signals with the new state.
    func setSyncStrategy(_ account: String, _ strategy: SyncStrategyKind) {
        app?.setSyncStrategy(account: account, strategy: strategy)
    }

    /// Set an account's background-poll interval (minutes; snapped to the allowed set).
    func setPollInterval(_ account: String, _ minutes: UInt16) {
        app?.setPollInterval(account: account, minutes: minutes)
    }

    /// Subscribe or unsubscribe one folder for push on an account (capped in the core).
    func setPushFolder(_ account: String, _ folder: String, _ subscribed: Bool) {
        app?.setPushFolder(account: account, folder: folder, subscribed: subscribed)
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

    /// Reset the local cache and re-fetch everything (destructive). The observer fires
    /// when the re-sync lands.
    func reset() { app?.reset() }

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

    /// One account's folders, as rows for the sidebar tree, each carrying an identity unique
    /// across the **whole** pane rather than within its own account.
    ///
    /// Empty for an account the snapshot has no folders for yet (a first sync still running).
    func folderRows(for account: String) -> [SidebarFolder] {
        let folders = accountFolders.first { $0.accountId == account }?.folders ?? []
        return folders.map { SidebarFolder(account: account, folder: $0) }
    }

    /// The email of the account `id`, for display (falls back to the id itself).
    func accountEmail(_ id: String) -> String {
        accounts.first { $0.id == id }?.email ?? id
    }

    /// Removes account `id`: drops it from the running app (the core rebuilds the switcher +
    /// list without it and, if it was selected, falls back to the unified inbox). The core also
    /// erases its Keychain entry, through the same port it wrote it through, so it doesn't return
    /// on the next launch.
    func removeAccount(_ id: String) {
        do {
            try app?.removeAccount(id: id)
        } catch {
            // The account IS gone from the app, only the Keychain erase failed, so it would come
            // back at the next launch. Say so rather than letting it reappear unexplained.
            FileLog.shared.append(
                level: "WARN",
                target: "accounts",
                message: "remove account: the stored credential could not be erased: \(error)"
            )
        }
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

    /// Sets and persists the default reply/forward quote style, updating the mirror at once so
    /// the settings picker reflects the choice without waiting for the `Surface::Settings` signal.
    func setQuoteStyle(_ style: QuoteStyleKind) {
        quoteSettings = QuoteSettings(style: style, perMessage: quoteSettings.perMessage)
        app?.setQuoteStyle(style: style)
    }

    /// Records the usage-statistics decision, and it *is* a decision either way: passing `false`
    /// stores a decline, which is what stops us asking again. Opting in mints the install id;
    /// opting out clears it and asks the backend to erase what it holds (GDPR Art. 17).
    ///
    /// The mirror is updated from the core's own answer rather than from `enabled`, so the UI can
    /// never disagree with what was actually persisted. The core raises no `Surface::Settings` for
    /// this, so without the mirror the toggle would snap back until the next launch.
    func setAnalyticsConsent(_ enabled: Bool) {
        app?.setAnalyticsConsent(enabled: enabled)
        analyticsConsent = app?.analyticsConsent()
    }

    /// The literal JSON the core would put on the wire, for the "see exactly what we send" panel.
    func analyticsPayloadPreview() -> String {
        app?.analyticsPayloadPreview() ?? ""
    }

    /// Sets and persists whether the composer offers a per-message quote-style override. Off by
    /// default: an ordinary reply just uses the default style above and shows no picker.
    func setQuoteStylePerMessage(_ perMessage: Bool) {
        quoteSettings = QuoteSettings(style: quoteSettings.style, perMessage: perMessage)
        app?.setQuoteStylePerMessage(perMessage: perMessage)
    }

    /// Sets and persists the app-level default send account (`nil` clears it, restoring "the
    /// first configured account"), updating the mirror at once as `setQuoteStyle` does.
    func setDefaultSendAccount(_ id: String?) {
        defaultSendAccount = id
        app?.setDefaultSendAccount(account: id)
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

    /// Sets and persists what one swipe direction does. The two directions are independent.
    func setSwipeAction(_ direction: SwipeDirection, _ action: SwipeActionKind) {
        switch direction {
        case .left: swipeSettings.left = action
        case .right: swipeSettings.right = action
        }
        app?.setSwipeAction(direction: direction, action: action)
    }

    /// The account the composer's From dropdown opens on, resolved against the accounts that
    /// actually exist. `preferred` is the context's own choice, the account that received the
    /// mail being replied to/forwarded, or the selected mailbox's account for a new message.
    /// Falling back: the app-level default send account, then the first configured one. A stored
    /// default naming a removed account therefore degrades to the first rather than to nothing,
    /// which is the core's own resolution order (`App::compose_account`).
    func sendAccount(preferring preferred: String?) -> AccountRow? {
        for candidate in [preferred, defaultSendAccount] {
            if let candidate, let match = accounts.first(where: { $0.id == candidate }) {
                return match
            }
        }
        return accounts.first
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

    /// Archive a whole conversation, move every message on the thread to Archive **except**
    /// those in the Sent folder (a sent copy never leaves Sent), then re-sync. The core decides
    /// which messages qualify (it knows the folder roles).
    func archiveThread(_ account: String, _ threadId: String) {
        app?.dispatch(intent: .archiveThread(account: account, threadId: threadId))
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

    /// Turns the local MCP server on or off. Persisted and applied at once, so the socket appears
    /// or disappears with the switch rather than at the next launch.
    func setMcpEnabled(_ enabled: Bool) {
        app?.setMcpEnabled(enabled: enabled)
        mcpSettings = app?.mcpSettings()
    }

    /// Shares one account with assistants, or stops. Unticking revokes access on the running
    /// server immediately, it does not wait for a restart.
    func setMcpAccountExposed(_ account: String, _ exposed: Bool) {
        app?.setMcpAccountExposed(account: account, exposed: exposed)
        mcpSettings = app?.mcpSettings()
    }

    /// Lets assistants send mail with no human review, or not. With it off the send tool is
    /// absent from what an assistant can even see.
    func setMcpAllowDirectSend(_ allow: Bool) {
        app?.setMcpAllowDirectSend(allow: allow)
        mcpSettings = app?.mcpSettings()
    }

    /// Restricts a direct send to people the user already emails, or lifts that restriction.
    func setMcpRequireKnownRecipient(_ require: Bool) {
        app?.setMcpRequireKnownRecipient(require: require)
        mcpSettings = app?.mcpSettings()
    }
}
