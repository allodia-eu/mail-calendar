// What an Outbox row offers in each state, and which intent each offer sends (`docs/sending.md`).
//
// Both fail silently in the running app: a Send Now on a row whose delivery was not confirmed
// is how a message arrives twice, and a Send Again that dispatches the wrong intent is refused
// by the core with nothing on screen to say so.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct OutboxRowActionTests {

    private let account = "acct-1"
    private let op: UInt64 = 7
    private let staging = "/tmp/staging"

    private func intent(_ action: OutboxRowAction) -> OutboxIntent {
        action.intent(account: account, op: op, stagingDirectory: staging)
    }

    private func row(_ state: QueuedState, editable: Bool) -> QueuedRow {
        QueuedRow(
            account: account,
            op: op,
            to: "ada@example.test",
            subject: "Terms",
            state: state,
            attempts: 1,
            detail: nil,
            editable: editable
        )
    }

    @Test func eachStateOffersItsActionsInOrder() {
        #expect(OutboxRowAction.offered(for: .waiting, editable: true) == [.sendNow, .edit, .cancelSend])
        #expect(OutboxRowAction.offered(for: .sending, editable: true).isEmpty)
        #expect(OutboxRowAction.offered(for: .unconfirmed, editable: true) == [.markSent, .confirmNotSent])
        #expect(OutboxRowAction.offered(for: .notSent, editable: true) == [.sendAgain, .edit, .discard])
    }

    @Test func aSendNoComposerCanHoldIsNeverOfferedEdit() {
        // An invitation's answer: the core refuses to edit it, so offering Edit would be a menu
        // item that does nothing.
        #expect(OutboxRowAction.offered(for: .waiting, editable: false) == [.sendNow, .cancelSend])
        #expect(OutboxRowAction.offered(for: .sending, editable: false).isEmpty)
        #expect(OutboxRowAction.offered(for: .unconfirmed, editable: false) == [.markSent, .confirmNotSent])
        #expect(OutboxRowAction.offered(for: .notSent, editable: false) == [.sendAgain, .discard])
    }

    @Test func aRowIsOfferedWhatItsStateAndEditabilityDecide() {
        #expect(OutboxRowAction.offered(for: row(.notSent, editable: true)) == [.sendAgain, .edit, .discard])
        #expect(OutboxRowAction.offered(for: row(.notSent, editable: false)) == [.sendAgain, .discard])
        #expect(OutboxRowAction.offered(for: row(.waiting, editable: false)) == [.sendNow, .cancelSend])
    }

    @Test func anUnconfirmedRowNeverOffersToSendEditOrCancel() {
        let offered = OutboxRowAction.offered(for: .unconfirmed, editable: true)
        for action in [OutboxRowAction.sendNow, .sendAgain, .edit, .cancelSend, .discard] {
            #expect(!offered.contains(action))
        }
    }

    @Test func eachActionSendsItsIntent() {
        #expect(intent(.sendNow) == .sendNow(account: account, op: op))
        #expect(intent(.edit) == .edit(account: account, op: op, stagingDirectory: staging))
        #expect(intent(.cancelSend) == .cancel(account: account, op: op))
        #expect(intent(.markSent) == .confirmSent(account: account, op: op))
        #expect(intent(.discard) == .cancel(account: account, op: op))
    }

    @Test func sendAgainIsAnAnswerOnAnUnconfirmedRowAndASendOnARefusedOne() {
        let unconfirmed = OutboxRowAction.offered(for: .unconfirmed, editable: true).map(intent)
        let refused = OutboxRowAction.offered(for: .notSent, editable: true).map(intent)
        #expect(unconfirmed.contains(.confirmNotSent(account: account, op: op)))
        #expect(!unconfirmed.contains(.sendNow(account: account, op: op)))
        #expect(refused.contains(.sendNow(account: account, op: op)))
        #expect(!refused.contains(.confirmNotSent(account: account, op: op)))
    }

    @Test func onlySendAgainOnAnUnconfirmedRowAsksFirst() {
        // It may deliver the message a second time. Send Again on a refused message is an
        // ordinary send of something that did not go, and asks nothing.
        #expect(OutboxRowAction.confirmNotSent.needsConfirmation)
        for action in [OutboxRowAction.sendNow, .edit, .cancelSend, .markSent, .sendAgain, .discard] {
            #expect(!action.needsConfirmation)
        }
    }

    @Test func onlyCancelAndDiscardAreDestructive() {
        #expect(OutboxRowAction.cancelSend.isDestructive)
        #expect(OutboxRowAction.discard.isDestructive)
        for action in [OutboxRowAction.sendNow, .edit, .markSent, .confirmNotSent, .sendAgain] {
            #expect(!action.isDestructive)
        }
    }
}
