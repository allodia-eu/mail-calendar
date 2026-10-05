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
    /// Move the message back into Drafts and open it in the composer. The core saves it as a
    /// draft and withdraws it from the Outbox, then offers it back through
    /// `Surface::ComposeRequest`, which the shell opens as a resumed draft.
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

    /// What `row` offers, in the order it shows them.
    static func offered(for row: QueuedRow) -> [OutboxRowAction] {
        offered(for: row.state, editable: row.editable)
    }

    /// What a row in `state` offers, in the order it shows them.
    ///
    /// A message on its way offers nothing: it cannot be called back. One whose delivery was
    /// not confirmed may already be with its recipients, so it offers only the two answers and
    /// never Send Now, Edit or Cancel. A send no composer can hold (an invitation's answer) is
    /// never offered Edit.
    static func offered(for state: QueuedState, editable: Bool) -> [OutboxRowAction] {
        let actions: [OutboxRowAction] = switch state {
        case .waiting: [.sendNow, .edit, .cancelSend]
        case .sending: []
        case .unconfirmed: [.markSent, .confirmNotSent]
        case .notSent: [.sendAgain, .edit, .discard]
        }
        return editable ? actions : actions.filter { $0 != .edit }
    }

    /// Whether the action loses the message, and is styled and placed as such.
    var isDestructive: Bool {
        switch self {
        case .cancelSend, .discard: true
        case .sendNow, .edit, .markSent, .confirmNotSent, .sendAgain: false
        }
    }

    /// Whether the user is asked before the action is sent. Only the answer that may deliver a
    /// message twice is; Send Again on a refused message is an ordinary send.
    var needsConfirmation: Bool {
        switch self {
        case .confirmNotSent: true
        case .sendNow, .edit, .cancelSend, .markSent, .sendAgain, .discard: false
        }
    }

    /// The intent this action dispatches for the queued message `op` in `account`.
    /// `stagingDirectory` is where an edit's files are written for the composer to attach.
    func intent(account: String, op: UInt64, stagingDirectory: String) -> OutboxIntent {
        switch self {
        case .sendNow, .sendAgain: .sendNow(account: account, op: op)
        case .edit: .edit(account: account, op: op, stagingDirectory: stagingDirectory)
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

    /// Carries out `action` on `row`. The core decides; this only asks. An action that
    /// `needsConfirmation` reaches here only once the user has confirmed it.
    func performOutboxAction(_ action: OutboxRowAction, on row: QueuedRow) {
        let intent = action.intent(
            account: row.account,
            op: row.op,
            stagingDirectory: draftStagingDirectory().path
        )
        app?.dispatch(intent: .outbox(intent: intent))
    }
}
