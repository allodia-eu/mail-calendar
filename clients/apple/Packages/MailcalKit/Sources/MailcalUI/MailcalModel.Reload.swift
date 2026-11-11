// The snapshot pump: one `Surface` signal from Rust in, the matching published state out.
// Split out of MailcalModel.swift so each file stays under 500 lines; the model object is
// defined there and this extends it.

import MailcalBindings

extension MailboxModel {
    func reload(_ surface: Surface) {
        // Notices touch nothing else but Settings → Accounts, which says when a provider refused a
        // use, a sign-in expired, or a calendar or address book did not connect; the accounts
        // snapshot is published only when it changed. Returning keeps the signal from the
        // mailbox reload below.
        if case .notices = surface {
            reloadNotices()
            refreshAccountsSnapshot()
            return
        }
        // A send signal only updates the "sending…" → "sent" hint; it doesn't touch the
        // mailbox/calendar projection (the post-send refresh fires its own signal). The core
        // owns the terminal `.sent`/`.failed` → `.idle` auto-clear (and its staleness guard),
        // delivering the reset as a later `.sending` signal, so we just publish what it reports.
        if case .sending = surface {
            sendStatus = app?.sendStatus() ?? .idle
            return
        }
        // A draft-save signal names no composition, so there is nothing to publish but the
        // fact that one moved: every open composer re-pulls its own state off this counter
        // (`docs/drafts.md`). It touches neither the projection nor any other composer.
        if case .draftStatus = surface {
            draftStatusVersion &+= 1
            return
        }
        // A sync-progress signal only updates the download bar; the rows it commits arrive on
        // their own MailboxList signal, so it doesn't touch the projection.
        if case .syncProgress = surface {
            syncProgress = app?.syncProgress()
            return
        }
        // A calendar-write-status signal only moves the small header badge (spinner → check /
        // warning); the grid/agenda arrive on their own `.calendar` signal.
        if case .calendarStatus = surface {
            calendarWriteStatus = app?.calendarWriteStatus() ?? .idle
            return
        }
        // A pass over the other devices starting or ending moves only the Accounts card and the
        // first-run card, which both read `allodiaDevices`; the rest of Settings stays as it is.
        if case .otherDevices = surface {
            refreshAllodiaDevices()
            return
        }
        // A queued message the user asked to edit. The core has moved it back into Drafts,
        // withdrawn it from the Outbox and joined it to the composition the opening names, so it
        // opens through the same context every composer does: the composer saves over that copy,
        // and its send takes it away (`docs/drafts.md`). Then the core is told the request is
        // taken.
        if case .composeRequest = surface {
            if let opening = app?.composeRequest() {
                pendingOutboxEdit = composeContext(for: opening)
                app?.dispatch(intent: .dismissComposeRequest)
            }
            return
        }
        // A settings signal only updates the settings surfaces (the timezone prompt, the accounts
        // and the page open, and the app-level composing/swipe preferences); it doesn't change the
        // mailbox/calendar projection.
        if case .settings = surface {
            timezone = app?.timezoneSettings()
            quoteSettings = app?.quoteSettings() ?? QuoteSettings(style: .indented, perMessage: false)
            defaultSendAccount = app?.defaultSendAccount()
            swipeSettings = app?.swipeSettings() ?? SwipeSettings(left: .delete, right: .delete)
            signatures = app?.signatures() ?? SignaturesSnapshot(signatures: [], accounts: [])
            mcpSettings = app?.mcpSettings()
            if let settings = app?.displaySettings() { displaySettings = settings }
            refreshAccountsSnapshot()
            return
        }
        // A calendar signal only updates the agenda, pull just the events, not the mailbox
        // list. (Keeping this off the hot mailbox path below matters during a sync.)
        if case .calendar = surface {
            // The agenda list is a snapshot; the GRID is a pull, so the version bump is what tells it
            // to re-read whatever page is on screen.
            events = app?.calendarList().events ?? []
            calendarVersion &+= 1
            return
        }
        // A contacts signal only updates the people list, the core has already applied the
        // active search query, so this one read covers both "a sync landed" and "the user typed
        // in the search field" without the host tracking which it was.
        // The last snapshot is kept if there is nothing to read, rather than blanked: an empty
        // Contacts screen says "no contacts yet", which reads as *your contacts are gone*, the
        // exact misreading its two distinct empty states exist to avoid. (The core keeps the last
        // snapshot on a failed store read for the same reason.)
        // A write-status signal only moves the line under the contacts search field; the list
        // arrives on its own `.contacts` signal. Its own branch rather than falling through to
        // the mailbox path below, which would re-pull every row for a word.
        if case .contactsStatus = surface {
            contactWriteStatus = app?.contactWriteStatus() ?? .idle
            return
        }
        if case .contacts = surface {
            if let snapshot = app?.contactList() { contacts = snapshot.rows }
            return
        }
        // The hot path: a MailboxList signal (fired repeatedly during a sync). Update ONLY the
        // mailbox-list state. The calendar, timezone, and quote style have their own surfaces
        // (`.calendar` / `.settings`); re-pulling them on every mailbox update would re-render
        // unrelated views and jank scroll for nothing.
        let snapshot = app?.mailboxList()
        rows = snapshot?.rows ?? []
        // Record the full count and release the "show more" guard: this snapshot answers any
        // in-flight request, so the list may ask for the next page as it scrolls again.
        total = snapshot?.total ?? 0
        loadMorePending = false
        mode = snapshot?.mode ?? .flat
        accounts = snapshot?.accounts ?? []
        selectedAccount = snapshot?.selectedAccount
        folders = snapshot?.folders ?? []
        accountFolders = snapshot?.accountFolders ?? []
        unifiedUnread = snapshot?.unifiedUnread ?? 0
        // Only from a real snapshot: before the core opens there is none, and a zero then would
        // clear the badge the last run left until the first snapshot put it back.
        if let snapshot { appBadge.show(snapshot.unifiedUnread) }
        unifiedExpanded = snapshot?.unifiedExpanded ?? true
        outbox = snapshot?.outbox ?? []
        showingOutbox = snapshot?.showingOutbox ?? false
        showingDrafts = snapshot?.showingDrafts ?? false
        selected = snapshot?.selected
        searchHorizon = snapshot?.searchHorizon
        emptyReason = snapshot?.emptyReason
        // The reading body (a potentially large HTML string) only changes on a Reading
        // signal, pull it just then, not on every mailbox refresh. The signal names no reader,
        // so every open detached window re-reads its own slot alongside the pane
        // (`docs/reading-window.md`).
        if case .reading = surface {
            reading = app?.readingView()
            reloadReadingWindows()
        }
        #if os(macOS)
        // A mailbox signal is the live runtime saying it committed mail, so it is also the only
        // honest moment to ask whether any of it is worth a notification: the desktop raises them
        // off its own IDLE/poll cadence rather than a schedule of its own
        // (docs/background-sync.md). Off the main actor, and collapsed when they burst
        // (MailcalModel.Notifications.swift). Named exactly, not "whatever reached the hot path":
        // a reading signal gets here too, and an open message is not mail arriving.
        if case .mailboxList = surface { collectNewMail() }
        #endif
        // Counts only in the log; content renders on screen, not in stdout.
        print("[Mailcal] rendered \(rows.count) rows (\(mode))")
    }
}
