// The shell's way into a detached reading window (`docs/reading-window.md`): a double-click on a
// message row, and the menu item that gives the same thing a name.
//
// macOS only. The iPhone and the iPad have one window by construction, and the contract does not
// ask them for a second.

#if os(macOS)
import MailcalBindings
import SwiftUI

extension ContentView {
    /// Opens a list row in its own window.
    func openInWindow(_ message: FlatRow) {
        openInWindow(opened(message))
    }

    /// Opens one message of an expanded conversation in its own window.
    func openInWindow(_ thread: ThreadRow, _ message: ThreadMessage) {
        openInWindow(opened(message, subject: thread.subject))
    }

    /// Registers the window with the model (which is what asks the core for its body) and asks
    /// the system for the window itself.
    ///
    /// Both are keyed by the message, so the second double-click on a row that is already open
    /// brings its window forward rather than opening a duplicate: the model's registration
    /// already exists and the scene value already has a window.
    private func openInWindow(_ message: OpenedMessage) {
        model.openReadingWindow(message)
        openWindow(
            id: DesktopWindow.reading,
            value: ReadingWindowID(account: message.account, key: message.key)
        )
    }
}
#endif
