// The Outbox's actions, as the sidebar row and the queued list call them.
//
// Each names a queued message by its account **and** its op id together: an op id is unique
// only within its own account's queue, and this list holds every account's at once
// (`docs/sending.md`).

import Foundation
import MailcalBindings

/// One thing a person can do about an unsent message, as its Outbox row offers it.
///
/// Two of them read "Send Again" and are different answers. On a refused message it is an
/// ordinary send; on one whose delivery was not confirmed it is the user saying it did not
/// arrive, which is the only way such a message is ever sent a second time.
enum OutboxRowAction: Hashable {
    /// Send a waiting message now instead of waiting out its backoff.
    case sendNow
    /// Withdraw the message and reopen it in the composer. The core withdraws it first and
    /// then offers it back through `Surface::ComposeRequest`, which `MailcalModel.Composer`
    /// opens.
    case edit
    /// Withdraw a waiting message so it is never delivered.
    case cancelSend
    /// The user says an unconfirmed message was delivered: it leaves the Outbox.
    case markSent
    /// The user says an unconfirmed message was not delivered: it is sent again now.
    case confirmNotSent
    /// Send a message the server refused once more.
    case sendAgain
    /// Drop a message the server refused.
    case discard

    /// What a row in `state` offers, in the order it shows them.
    ///
    /// A message on its way offers nothing: it cannot be called back. One whose delivery was
    /// not confirmed may already be with its recipients, so it offers only the two answers and
    /// never Send Now, Edit or Cancel.
    static func offered(for state: QueuedState) -> [OutboxRowAction] {
        switch state {
        case .waiting: [.sendNow, .edit, .cancelSend]
        case .sending: []
        case .unconfirmed: [.markSent, .confirmNotSent]
        case .notSent: [.sendAgain, .edit, .discard]
        }
    }

    /// Whether the action loses the message, and is styled and placed as such.
    var isDestructive: Bool {
        switch self {
        case .cancelSend, .discard: true
        case .sendNow, .edit, .markSent, .confirmNotSent, .sendAgain: false
        }
    }

    /// The intent this action dispatches for the queued message `op` in `account`.
    func intent(account: String, op: UInt64) -> OutboxIntent {
        switch self {
        case .sendNow, .sendAgain: .sendNow(account: account, op: op)
        case .edit: .edit(account: account, op: op)
        case .cancelSend, .discard: .cancel(account: account, op: op)
        case .markSent: .confirmSent(account: account, op: op)
        case .confirmNotSent: .confirmNotSent(account: account, op: op)
        }
    }
}

extension MailboxModel {
    /// Shows the Outbox: every account's unsent messages, in one list.
    func showOutbox() {
        destination = .mail
        app?.dispatch(intent: .outbox(intent: .show))
    }

    /// Carries out `action` on `row`. The core decides; this only asks.
    func performOutboxAction(_ action: OutboxRowAction, on row: QueuedRow) {
        app?.dispatch(intent: .outbox(intent: action.intent(account: row.account, op: row.op)))
    }
}
