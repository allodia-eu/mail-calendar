// The snapshot pump: one `Surface` signal from Rust in, the matching published state out.
// Split out of MailcalModel.swift so each file stays under 500 lines; the model object is
// defined there and this extends it.

import MailcalBindings

extension MailboxModel {
    func reload(_ surface: Surface) {
        // A send signal only updates the "sending…" → "sent" hint; it doesn't touch the
        // mailbox/calendar projection (the post-send refresh fires its own signal). The core
        // owns the terminal `.sent`/`.failed` → `.idle` auto-clear (and its staleness guard),
        // delivering the reset as a later `.sending` signal, so we just publish what it reports.
        if case .sending = surface {
            sendStatus = app?.sendStatus() ?? .idle
            return
        }
        // A sync-progress signal only updates the download bar; the rows it commits arrive on
        // their own MailboxList signal, so it doesn't touch the projection.
        if case .syncProgress = surface {
            syncProgress = app?.syncProgress()
            return
        }
        // A connectivity signal only updates the offline banner + per-account outage badges.
        if case .connectivity = surface {
            connectivity = app?.connectivity()
            return
        }
        // A calendar-write-status signal only moves the small header badge (spinner → check /
        // warning); the grid/agenda arrive on their own `.calendar` signal.
        if case .calendarStatus = surface {
            calendarWriteStatus = app?.calendarWriteStatus() ?? .idle
            return
        }
        // An invitation-reply signal only raises or clears the "the organiser wasn't told"
        // modal. It arrives in both directions, the core clears the question as soon as it is
        // answered, so this assigns whatever the core now holds rather than only setting it.
        if case .invitationReply = surface {
            replyPrompt = app?.replyPrompt()
            return
        }
        // The missing-Sent-copy question has its own surface, and the core signals it in both
        // directions too, it clears the question the moment the copy is filed or the user
        // dismisses it, so this assigns whatever the core now holds, and `nil` is what closes
        // the sheet.
        if case .unfiledCopy = surface {
            unfiledCopy = app?.unfiledCopy()
            return
        }
        // A settings signal only updates the settings surfaces (the timezone prompt, the
        // per-account sync behaviour, and the app-level composing/swipe preferences); it doesn't
        // change the mailbox/calendar projection.
        if case .settings = surface {
            timezone = app?.timezoneSettings()
            syncSettings = app?.syncSettings()
            quoteSettings = app?.quoteSettings() ?? QuoteSettings(style: .indented, perMessage: false)
            defaultSendAccount = app?.defaultSendAccount()
            swipeSettings = app?.swipeSettings() ?? SwipeSettings(left: .delete, right: .delete)
            signatures = app?.signatures() ?? SignaturesSnapshot(signatures: [], accounts: [])
            mcpSettings = app?.mcpSettings()
            if let settings = app?.displaySettings() { displaySettings = settings }
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
        selected = snapshot?.selected
        searchHorizon = snapshot?.searchHorizon
        // The reading body (a potentially large HTML string) only changes on a Reading
        // signal, pull it just then, not on every mailbox refresh.
        if case .reading = surface { reading = app?.readingView() }
        // Counts only in the log; content renders on screen, not in stdout.
        print("[Mailcal] rendered \(rows.count) rows (\(mode))")
    }
}
