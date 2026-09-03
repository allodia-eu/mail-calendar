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
        CoreSurface.MAILBOX_LIST -> reload()
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
        // The answer is already saved; what failed is the message to the organiser.
        CoreSurface.INVITATION_REPLY -> replyPrompt = app.replyPrompt()
        // The message went out; its copy did not reach Sent, and nothing later
        // will find it. The user is offered the one repair there is.
        CoreSurface.UNFILED_COPY -> unfiledCopy = app.unfiledCopy()
        // A settings change refreshes the per-account sync-behaviour screen and the
        // default quote style (the timezone snapshot the caller took already covers
        // the pending-zone prompt).
        CoreSurface.SETTINGS -> {
            syncSettings = app.syncSettings()
            quoteSettings = app.quoteSettings()
            swipeSettings = app.swipeSettings()
            defaultSendAccount = app.defaultSendAccount()
            displaySettings = app.displaySettings()
            signatures = app.signatures()
        }
        // The reading body (a potentially large HTML string) only changes on a
        // Reading signal, pull it just then, not on every other refresh.
        CoreSurface.READING -> reading = app.readingView()
        CoreSurface.SENDING -> updateSendStatus(app.sendStatus())
        // A sync-progress signal only updates the download bar; the rows it commits
        // arrive on their own MAILBOX_LIST signal.
        CoreSurface.SYNC_PROGRESS -> syncProgress = app.syncProgress()
        // A connectivity signal updates the offline banner, the per-account outage
        // badges, and the friendly connection-issues banner (names + details).
        CoreSurface.CONNECTIVITY -> refreshConnectivity()
    }
}
