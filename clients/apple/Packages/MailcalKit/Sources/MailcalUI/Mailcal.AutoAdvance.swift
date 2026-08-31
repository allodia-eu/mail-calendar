// Where the reading pane lands when the message in it leaves the folder.
//
// Archiving or deleting from the reading view used to empty the pane, so working through a mailbox
// meant going back to the list and clicking the next message every single time. It now opens the
// next one down, and, when the message was the last in the list, the one above it, so clearing out
// the bottom of a folder doesn't dump the reader on a placeholder with a full mailbox behind it.
//
// The choice is a pure function over the rows the list is showing, deliberately: it is the whole of
// the behaviour, and a unit test can pin it without composing a view or touching the core.

import MailcalBindings
import SwiftUI

/// The message to open once `removed` is gone: the next one down, else the one above, else nothing.
///
/// `stops` is the list **as it is on screen right now**, so this must be called *before* the
/// archive/delete is dispatched, while `removed` is still in it. A `removed` that isn't in `stops`
/// (the folder changed under the reader, say) answers `nil`, which empties the pane as before.
func messageAfterRemoving(_ removed: OpenedMessage, from stops: [OpenedMessage]) -> OpenedMessage? {
    guard
        let index = stops.firstIndex(where: {
            $0.account == removed.account && $0.key == removed.key
        })
    else { return nil }
    if stops.indices.contains(index + 1) { return stops[index + 1] }
    if stops.indices.contains(index - 1) { return stops[index - 1] }
    return nil
}

extension ContentView {
    /// Whether a reading **pane** is on screen beside the list, which is what makes advancing sensible.
    ///
    /// On the iPhone the reading view is a pushed screen, not a pane: clearing pops back to the list,
    /// which is the whole window and the right place to be. macOS and the iPad's regular width both
    /// keep the list visible next to the pane, so the next message can simply appear in it.
    ///
    /// The test is the **size class**, the same one `baseLayout` branches on, not the device idiom.
    /// An iPad in a narrow multitasking split reads as compact and gets the iPhone layout, so it has
    /// no pane to advance in either. (Also used by the showcase driver, Mailcal.Showcase.swift.)
    var hasReadingPane: Bool {
        #if os(macOS)
        return true
        #else
        return hSize != .compact
        #endif
    }

    /// The messages the list is showing, in the order they appear, a flat row is itself, an expanded
    /// conversation contributes its own messages in place of its header, and a collapsed one stands
    /// for the single message its row summarises (the same one tapping it would open).
    var readableStops: [OpenedMessage] {
        visibleRows.flatMap { row -> [OpenedMessage] in
            switch row {
            case .flat(let message):
                return [opened(message)]
            case .thread(let thread):
                if expandedThreads.contains(threadKey(thread)) {
                    return thread.messages.map { opened($0, subject: thread.subject) }
                }
                let representative = thread.messages.first { $0.key == thread.latestKey }
                    ?? thread.messages.first
                return representative.map { [opened($0, subject: thread.subject)] } ?? []
            }
        }
    }

    /// Where the pane should land after `opened` leaves the folder, read **before** dispatching the
    /// action, so the answer doesn't depend on whether the core has re-projected the list yet.
    func stopAfterRemoving(_ opened: OpenedMessage) -> OpenedMessage? {
        guard hasReadingPane else { return nil }
        return messageAfterRemoving(opened, from: readableStops)
    }

    /// Opens what `stopAfterRemoving` chose, or empties the pane when there was nothing left.
    func settleReadingPane(_ next: OpenedMessage?) {
        guard let next else {
            clearOpenedMessage()
            return
        }
        openNow(next)
    }

    func opened(_ message: FlatRow) -> OpenedMessage {
        OpenedMessage(
            account: message.account,
            key: message.key,
            subject: message.subject,
            from: message.from,
            avatar: message.avatar,
            date: localDateTime(message.date, in: model.activeZone)
        )
    }

    func opened(_ message: ThreadMessage, subject: String) -> OpenedMessage {
        OpenedMessage(
            account: message.account,
            key: message.key,
            subject: subject,
            from: message.from,
            avatar: message.avatar,
            date: localDateTime(message.date, in: model.activeZone)
        )
    }

    /// Records the header and asks the core for the body, the unguarded open, shared by the row taps
    /// and by the auto-advance (which has no draft to guard: the pane it is replacing is a reader).
    func openNow(_ message: OpenedMessage) {
        setOpenedMessage(message)
        model.openMessage(message.account, message.key)
    }
}
