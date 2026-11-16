// Which pull answers which signal: the core says a surface moved, and this decides what to read
// back. One `when` over every surface, so the compiler names the arm anyone forgets to add.
//
// A signal is not a payload. Each arm reads exactly the snapshot its surface covers and nothing
// else, which is why the narrow status surfaces exist: a write badge must not drag a mailbox list
// across the FFI, and a list must not wait on a badge.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.Surface as CoreSurface

internal fun MainActivity.pullFor(surface: CoreSurface, app: MailcalApp) {
    when (surface) {
        CoreSurface.MAILBOX_LIST -> {
            reload()
        }
        // A message the core withdrew from the Outbox so the user could change it. It exists
        // nowhere else by the time this arrives, so this may not drop it: it is held until the
        // composer has it, and only then is the request dismissed (docs/sending.md).
        CoreSurface.COMPOSE_REQUEST -> {
            withdrawnMessage = app.composeRequest()
        }
        // The agenda list is a snapshot; the grid is a pull, so the version bump is
        // what tells it to re-read whatever page is on screen.
        CoreSurface.CALENDAR -> {
            events = app.calendarList().events
            calendarVersion += 1
        }
        CoreSurface.CONTACTS -> contacts = app.contactList().rows
        // A write-status signal only moves the editor's own feedback; the list
        // arrives on its own CONTACTS signal.
        CoreSurface.CONTACTS_STATUS -> contactWrites.status = app.contactWriteStatus()
        // A write-status signal only moves the small header badge (spinner → check /
        // warning); the grid/agenda arrive on their own CALENDAR signal.
        CoreSurface.CALENDAR_STATUS -> calendarWriteStatus = app.calendarWriteStatus()
        // A setting read outside an account's page moved: the composing, swipe, display and
        // signature preferences, and the accounts every screen reads.
        CoreSurface.SETTINGS -> {
            quoteSettings = app.quoteSettings()
            swipeSettings = app.swipeSettings()
            defaultSendAccount = app.defaultSendAccount()
            displaySettings = app.displaySettings()
            signatures = app.signatures()
            // Every account, mail or not: adding, removing, linking and signing one in again all
            // raise this signal (docs/accounts.md rule 11).
            refreshAccountsSnapshot()
            refreshSeparate()
        }
        // A change on an account's page started, waited or ended, or a server answered what the
        // page asked: the page, the list and the questions it shares with other screens are read
        // again, and nothing else moves. What a change stores raises SETTINGS besides.
        CoreSurface.ACCOUNT_PAGE -> {
            refreshAccountsSnapshot()
            refreshSeparate()
            // An account's browser sign-in moved: its browser opens once the core waits on it.
            driveBrowserSignIn()
        }
        // A pass over the person's other devices started or ended: their card moves, and the
        // first-run card's progress with it.
        CoreSurface.OTHER_DEVICES -> {
            allodiaDevices = app.allodiaDevices()
            allodiaSignalled(app)
        }
        // The Allodia account's card moved: its sign-in or sign-out, or the service refusing the
        // stored sign-in.
        CoreSurface.ALLODIA -> allodiaSignalled(app)
        // The reading body (a potentially large HTML string) only changes on a
        // Reading signal, pull it just then, not on every other refresh.
        CoreSurface.READING -> reading = app.readingView()
        CoreSurface.SENDING -> updateSendStatus(app.sendStatus())
        // A sync-progress signal only updates the download bar; the rows it commits
        // arrive on their own MAILBOX_LIST signal.
        CoreSurface.SYNC_PROGRESS -> syncProgress = app.syncProgress()
        // A composition's save moved. The signal names none, so there is nothing to publish
        // but the fact that one did: every open composer re-pulls its own off this counter
        // (docs/drafts.md).
        CoreSurface.DRAFT_STATUS -> drafts.draftStatusVersion += 1
        // A provider refusing a calendar, or an account connecting, also changes a use's state on
        // the account's page, so the accounts snapshot is read again beside the notices.
        CoreSurface.NOTICES -> {
            pullNotices()
            refreshAccountsSnapshot()
            refreshSeparate()
        }
    }
}
