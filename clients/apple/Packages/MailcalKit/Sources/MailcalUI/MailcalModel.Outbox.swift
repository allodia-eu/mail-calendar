// The Outbox's actions, as the sidebar row and the queued list call them.
//
// Each names a queued message by its account **and** its op id together: an op id is unique
// only within its own account's queue, and this list holds every account's at once
// (`docs/sending.md`).

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Shows the Outbox: every account's unsent messages, in one list.
    func showOutbox() {
        destination = .mail
        app?.dispatch(intent: .outbox(intent: .show))
    }

    /// Sends a queued message now instead of waiting out its backoff.
    ///
    /// Not offered for a message that is already going out, or one whose delivery could not
    /// be confirmed: that one may already have reached its recipients, and asking again is
    /// how it arrives twice.
    func sendQueuedNow(_ row: QueuedRow) {
        app?.dispatch(intent: .outbox(intent: .sendNow(account: row.account, op: row.op)))
    }

    /// Withdraws a queued message so it is never delivered.
    func cancelQueued(_ row: QueuedRow) {
        app?.dispatch(intent: .outbox(intent: .cancel(account: row.account, op: row.op)))
    }

    /// Withdraws a queued message and reopens it in the composer.
    ///
    /// The core withdraws it first and then offers it back through `Surface::ComposeRequest`,
    /// which `MailcalModel.Composer` opens; this only asks.
    func editQueued(_ row: QueuedRow) {
        app?.dispatch(intent: .outbox(intent: .edit(account: row.account, op: row.op)))
    }

    /// Whether `row` may be acted on at all.
    ///
    /// A message in flight is on its way, and one awaiting confirmation may already have
    /// arrived; neither can be called back, so the row shows its state and offers nothing.
    func queuedRowIsActionable(_ row: QueuedRow) -> Bool {
        row.state == .waiting
    }
}
