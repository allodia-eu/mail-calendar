// New-mail notifications on macOS: the scan that feeds them, and the state that keeps it from
// piling up. Its own extension file so MailcalModel.swift stays under 500 lines.
//
// There is no second timer here, and that is the design (docs/background-sync.md). The desktop's
// live runtime, the standing IMAP IDLE watches and the poll each account is configured for,
// already delivers; a mailbox-list signal is the moment it committed something. So this reads the
// cache that signal published rather than starting a network pass of its own, which keeps
// notifications on the cadence the user chose in Settings rather than one this file invented.
// Windows and Linux reach the same core call the same way.
//
// iOS is not here: its process is suspended once it leaves the foreground, so there is no live
// runtime to hear from and a `BGAppRefreshTask` runs a bounded pass instead (BackgroundSync.swift).
#if os(macOS)
import MailcalBindings

/// Idle → inFlight → (pending) → idle. A sync raises mailbox signals in bursts, and the whole
/// burst behind a running scan collapses into one repeat, so the follow-up reads the settled cache
/// once rather than queueing a scan per signal.
enum NewMailScan {
    case idle, inFlight, pending
}

extension MailboxModel {
    /// Reports whatever inbound Inbox mail the live runtime has just committed, and raises a
    /// notification per message the core returns.
    ///
    /// The core withholds mail that arrived before this session started, so the catch-up sync a
    /// launch begins with is marked seen rather than announced, and a brand-new account is seeded
    /// rather than having its whole existing inbox announced. While new-mail notifications are
    /// off it returns nothing and still advances its marks, so turning them back on never floods
    /// with a backlog.
    func collectNewMail() {
        guard let app, !accounts.isEmpty else { return }
        guard newMailScan == .idle else {
            newMailScan = .pending
            return
        }
        newMailScan = .inFlight
        Task {
            // The core blocks its thread driving the runtime to completion, so the scan goes off
            // the main actor; what the user then sees is decided back on it.
            let outcome = await Task.detached(priority: .utility) {
                app.collectCachedNewMail()
            }.value
            await MailNotifier.notifyNewMail(outcome)
            let repeatScan = newMailScan == .pending
            newMailScan = .idle
            if repeatScan { collectNewMail() }
        }
    }
}
#endif
