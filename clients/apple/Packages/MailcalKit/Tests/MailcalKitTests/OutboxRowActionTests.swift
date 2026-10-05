// What an Outbox row offers in each state, and which intent each offer sends (`docs/sending.md`).
//
// Both fail silently in the running app: a Send Now on a row whose delivery was not confirmed
// is how a message arrives twice, and a Send Again that dispatches the wrong intent is refused
// by the core with nothing on screen to say so.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct OutboxRowActionTests {

    @Test func eachStateOffersItsActionsInOrder() {
        #expect(OutboxRowAction.offered(for: .waiting) == [.sendNow, .edit, .cancelSend])
        #expect(OutboxRowAction.offered(for: .sending).isEmpty)
        #expect(OutboxRowAction.offered(for: .unconfirmed) == [.markSent, .confirmNotSent])
        #expect(OutboxRowAction.offered(for: .notSent) == [.sendAgain, .edit, .discard])
    }

    @Test func anUnconfirmedRowNeverOffersToSendEditOrCancel() {
        let offered = OutboxRowAction.offered(for: .unconfirmed)
        for action in [OutboxRowAction.sendNow, .sendAgain, .edit, .cancelSend, .discard] {
            #expect(!offered.contains(action))
        }
    }

    @Test func eachActionSendsItsIntent() {
        let account = "acct-1"
        let op: UInt64 = 7
        #expect(OutboxRowAction.sendNow.intent(account: account, op: op) == .sendNow(account: account, op: op))
        #expect(OutboxRowAction.edit.intent(account: account, op: op) == .edit(account: account, op: op))
        #expect(OutboxRowAction.cancelSend.intent(account: account, op: op) == .cancel(account: account, op: op))
        #expect(OutboxRowAction.markSent.intent(account: account, op: op) == .confirmSent(account: account, op: op))
        #expect(OutboxRowAction.discard.intent(account: account, op: op) == .cancel(account: account, op: op))
    }

    @Test func sendAgainIsAnAnswerOnAnUnconfirmedRowAndASendOnARefusedOne() {
        let account = "acct-1"
        let op: UInt64 = 7
        let unconfirmed = OutboxRowAction.offered(for: .unconfirmed).map { $0.intent(account: account, op: op) }
        let refused = OutboxRowAction.offered(for: .notSent).map { $0.intent(account: account, op: op) }
        #expect(unconfirmed.contains(.confirmNotSent(account: account, op: op)))
        #expect(!unconfirmed.contains(.sendNow(account: account, op: op)))
        #expect(refused.contains(.sendNow(account: account, op: op)))
        #expect(!refused.contains(.confirmNotSent(account: account, op: op)))
    }

    @Test func onlyCancelAndDiscardAreDestructive() {
        #expect(OutboxRowAction.cancelSend.isDestructive)
        #expect(OutboxRowAction.discard.isDestructive)
        for action in [OutboxRowAction.sendNow, .edit, .markSent, .confirmNotSent, .sendAgain] {
            #expect(!action.isDestructive)
        }
    }
}
