import MailcalBindings
import Synchronization

/// A process-global **weak** handle to the foreground live core. A `BGAppRefreshTask` fires in the
/// still-resident (merely *suspended*, not terminated) app process, whose foreground core is alive
/// with the store already open, so the background pass **reuses** that instance instead of opening a
/// SECOND engine store + runtime over the same SQLite file (two write connections, duplicate IMAP).
/// Weak so it clears once the foreground model releases its core; a genuinely terminated app starts a
/// fresh process where this is `nil`, and the handler cold-builds a headless core. Set by
/// `MailcalModel.connect` (iOS only). See docs/background-sync.md.
///
/// Built on every platform although only iOS sets it, so `swift test` (macOS) can exercise it.
final class LiveCore: Sendable {
    static let shared = LiveCore()

    /// The weak handle, behind a `Mutex` rather than a lock beside it: the mutex owns the value, so
    /// there is no way to read the handle without holding it.
    private struct Handle: ~Copyable {
        weak var app: MailcalApp?
        /// The foreground core is being opened off the main actor (docs/boot-sequence.md), so a
        /// missing `app` does not yet mean the process has none.
        var opening = false
    }

    private let handle = Mutex(Handle())

    func beginOpening() {
        handle.withLock { $0.opening = true }
    }

    func openingFailed() {
        handle.withLock { $0.opening = false }
    }

    var isOpening: Bool {
        handle.withLock { $0.opening }
    }

    /// Returns once the foreground core has opened or failed to, or when the calling task is
    /// cancelled.
    func waitWhileOpening() async {
        while isOpening, !Task.isCancelled {
            try? await Task.sleep(for: .milliseconds(200))
        }
    }

    func set(_ app: MailcalApp?) {
        handle.withLock {
            $0.app = app
            $0.opening = false
        }
    }

    func current() -> MailcalApp? {
        handle.withLock { $0.app }
    }
}
