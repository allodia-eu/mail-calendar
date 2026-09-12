// The model's half of the detached reading window (`docs/reading-window.md`): which windows are
// open, the header each was opened on, and the body the core is holding for it.
//
// The window itself is a macOS scene (ReadingWindowScene.swift). This file has no `#if`, because
// nothing here is platform-specific: it is the same dispatch → snapshot loop the pane uses, aimed
// at a different slot, and a client that never opens a window simply never fills the map.

import MailcalBindings

/// One detached reading window's state: the row it was opened on, and the body once it lands.
///
/// The header travels with the window rather than being read back out of the body, so the window
/// draws a subject and a sender from its first frame, exactly as the pane does. Until the body
/// arrives there is nothing else to show, and a window that opened on a blank header would read
/// as broken for as long as the fetch takes.
struct DetachedReading {
    /// The list row the window was opened on: subject, sender, avatar, date.
    let message: OpenedMessage
    /// The fetched body, or `nil` while the open is still running.
    var body: ReadingSnapshot?
}

/// Where a reading view takes its body from. Two cases, because the core keeps two kinds of slot
/// and a window must never be able to read the pane's (`docs/reading-window.md`).
enum ReadingSource: Hashable {
    /// The reading pane, or the screen an iPhone pushes.
    case pane
    /// One detached reading window, by its core reader id.
    case window(String)
}

extension MailboxModel {
    /// The core reader id for a message's own window.
    ///
    /// Derived from the message rather than minted, which is what makes opening the same message
    /// twice reach the window that is already up instead of stacking a second one on top of it.
    /// The pair is unique: a provider key is unique within its account.
    static func readingWindowID(account: String, key: String) -> String { "\(account)/\(key)" }

    /// Opens `message` in its own window's slot, or, when that window is already up, leaves the
    /// body it holds alone: it is the same message, already fetched.
    ///
    /// The core does the rest, the fetch, the retry, the loading threshold and the mark-read are
    /// the pane's and are reached through the same intent (`docs/reading-window.md`).
    func openReadingWindow(_ message: OpenedMessage) {
        let id = Self.readingWindowID(account: message.account, key: message.key)
        guard readingWindows[id] == nil else { return }
        readingWindows[id] = DetachedReading(message: message)
        app?.dispatch(
            intent: .openMessageInWindow(window: id, account: message.account, key: message.key)
        )
    }

    /// The window has gone: forget its header and tell the core to drop its body.
    ///
    /// Both halves matter. A body is the largest thing either side holds per window, sanitised
    /// HTML with every inline image resolved into it, so leaving one behind is a leak that grows
    /// with each message the user opens.
    func closeReadingWindow(_ id: String) {
        readingWindows[id] = nil
        app?.closeReadingWindow(window: id)
    }

    /// Re-pulls every open window's body.
    ///
    /// A `Surface::Reading` signal says that *some* reader's body changed, not which, so each
    /// open window reads its own slot back. Called only on that signal, never on the mailbox
    /// path: a body is a large string and there is no reason to copy one per window per sync.
    func reloadReadingWindows() {
        guard let app else { return }
        for id in readingWindows.keys {
            readingWindows[id]?.body = app.readingWindowView(window: id)
        }
    }

    /// Closes every detached reading window's slot in the core.
    ///
    /// The main window going away takes its reading windows with it (`docs/reading-window.md`),
    /// and this is the core-side half of that: the host dismisses the windows, this frees what
    /// they were holding.
    func closeAllReadingWindows() {
        for id in readingWindows.keys {
            app?.closeReadingWindow(window: id)
        }
        readingWindows.removeAll()
    }
}
