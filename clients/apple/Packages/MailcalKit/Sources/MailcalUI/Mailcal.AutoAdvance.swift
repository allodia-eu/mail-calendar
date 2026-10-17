// Where the reading pane goes when the message in it leaves the list (`docs/reading-actions.md`,
// "Where the pane goes next"). Which message that is is the core's (`stop_after_removing`); this
// file asks it and opens the answer as clicking its row would, so a draft resumes in a composer.

import MailcalBindings
import SwiftUI

extension ContentView {
    /// Whether a reading **pane** is on screen beside the list, which is what makes moving on sensible.
    ///
    /// On the iPhone the reading view is a pushed screen, not a pane: clearing pops back to the list,
    /// which is the whole window and the right place to be. macOS and the iPad's regular width both
    /// keep the list visible next to the pane, so the next message can simply appear in it.
    ///
    /// The test is the **size class**, the same one `baseLayout` branches on, not the device idiom.
    /// An iPad in a narrow multitasking split reads as compact and gets the iPhone layout, so it has
    /// no pane to move on in either. (Also used by the showcase driver, Mailcal.Showcase.swift.)
    var hasReadingPane: Bool {
        #if os(macOS)
        return true
        #else
        return hSize != .compact
        #endif
    }

    /// Where the pane lands after `opened` leaves the list, asked **before** the action is
    /// dispatched, while `opened` is still listed. `nil` where there is no pane to move on in.
    func stopAfterRemoving(_ opened: OpenedMessage) -> MenuMessage? {
        guard hasReadingPane else { return nil }
        let expanded = expandedConversations.map {
            SelectedRow.thread(account: $0.account, threadId: $0.threadId)
        }
        return model.stopAfterRemoving(opened.account, opened.key, expanded: expanded)
    }

    /// Opens what `stopAfterRemoving` answered as clicking its row would, or empties the pane when
    /// nothing was left.
    ///
    /// A click keeps only the conversation it lands in expanded, which `openAsClicked` expands
    /// again. A draft resumes in a composer and leaves the pane empty behind it: the message the
    /// pane held has left the list.
    func settleReadingPane(_ next: MenuMessage?) {
        guard let next else {
            clearOpenedMessage()
            return
        }
        expandedThreads.removeAll()
        if next.kind == .draft { clearOpenedMessage() }
        openAsClicked(next)
    }

    func opened(_ message: FlatRow) -> OpenedMessage {
        OpenedMessage(
            account: message.account,
            key: message.key,
            subject: message.subject,
            from: message.from,
            avatar: message.avatar,
            date: localDateTime(message.date, in: model.activeZone, use24Hour: model.use24Hour)
        )
    }

    func opened(_ message: ThreadMessage, subject: String) -> OpenedMessage {
        OpenedMessage(
            account: message.account,
            key: message.key,
            subject: subject,
            from: message.from,
            avatar: message.avatar,
            date: localDateTime(message.date, in: model.activeZone, use24Hour: model.use24Hour)
        )
    }

    /// Records the header and asks the core for the body: the unguarded open behind the row taps.
    func openNow(_ message: OpenedMessage) {
        setOpenedMessage(message)
        model.openMessage(message.account, message.key)
    }
}
