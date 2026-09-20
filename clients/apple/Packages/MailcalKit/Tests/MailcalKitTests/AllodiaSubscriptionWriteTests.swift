// What may be done to a subscription, and what each ending is called, pinned as policy.
//
// Every one of these is about somebody's money, and each is a rule a screen could plausibly get
// backwards: offering to switch a subscription a store is billing, offering to restart one that
// has run out rather than been cancelled, confirming a cancellation without saying what is kept,
// or putting a refusal's machine-readable reason in front of a reader.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@MainActor
struct AllodiaSubscriptionWriteTests {
    /// ⚠️ `actions` decides, and nothing else. Somebody the App Store is charging has a period
    /// too, and switching that one through this API is exactly what the service refuses.
    @Test func onlyASubscriptionTheServiceSaysMaySwitchOffersIt() {
        #expect(allodiaSwitchTarget(subscription(own: own(.active), canSwitch: true)) == .monthly)
        #expect(allodiaSwitchTarget(subscription(own: own(.active), canSwitch: false)) == nil)
        // A store's subscription, which the service always refuses here.
        #expect(allodiaSwitchTarget(subscription(canSwitch: false)) == nil)
        // Permitted, but with nothing of Allodia's own to move.
        #expect(allodiaSwitchTarget(subscription(canSwitch: true)) == nil)
    }

    /// The target is simply the other period, in both directions.
    @Test func aSwitchMovesToTheOtherPeriod() {
        let monthly = own(.active, interval: .monthly)
        #expect(allodiaSwitchTarget(subscription(own: monthly, canSwitch: true)) == .yearly)
        let yearly = own(.active, interval: .yearly)
        #expect(allodiaSwitchTarget(subscription(own: yearly, canSwitch: true)) == .monthly)
    }

    /// ⚠️ Restarting and buying again are different things, and only the service knows which one
    /// applies: a cancelled subscription still inside its period restarts on the authorisation
    /// already held, and one whose period has run out is a new checkout.
    @Test func aRestartIsOfferedOnlyOnACancellationStillRunning() {
        #expect(allodiaOffersRestart(subscription(own: own(.cancelled), canResubscribe: true)))
        #expect(!allodiaOffersRestart(subscription(own: own(.active), canResubscribe: true)))
        #expect(!allodiaOffersRestart(subscription(own: own(.cancelled), canResubscribe: false)))
        #expect(!allodiaOffersRestart(subscription(canResubscribe: true)))
    }

    /// ⚠️ The date is the whole reassurance on the cancellation confirmation: without it the
    /// sentence reads as "it stops now", which is the one thing cancelling does not do.
    @Test func theConfirmationsTalkAboutWhatHasAlreadyBeenPaidFor() {
        #expect(allodiaWriteDate(subscription(own: own(.active))) == "2026-10-18T00:00:00Z")
        // No subscription of Allodia's own: the furthest date any source has been paid through.
        #expect(allodiaWriteDate(subscription()) == "2026-10-18T00:00:00Z")
        #expect(allodiaWriteDate(subscription(periodEnd: nil)) == "")
    }

    /// ⚠️ **A refusal is a code and never a message.** Rendering the error instead puts
    /// `String(reflecting:)` on screen, which reads
    /// `Refused(reason: MailcalBindings.AllodiaSubscriptionRefusal.notSwitchable)`.
    @Test func eachRefusalBecomesItsOwnReason() {
        let expected: [AllodiaSubscriptionRefusal: AllodiaWriteFailure] = [
            .alreadyCancelled: .alreadyCancelled,
            .alreadyActive: .alreadyActive,
            .alreadyOnInterval: .alreadyOnInterval,
            .notSwitchable: .notSwitchable,
            .notFound: .notFound,
        ]
        for (reason, failure) in expected {
            #expect(allodiaWriteFailure(AllodiaPurchaseError.Refused(reason: reason)) == failure)
        }
    }

    /// Nothing was learned, so there is nothing to explain, and the one thing worth saying is that
    /// nothing changed. A reason a later service invents lands here too, rather than being read as
    /// one this build happens to know.
    @Test func anythingThatIsNotARefusalSaysOnlyThatNothingChanged() {
        let errors: [AllodiaPurchaseError] = [
            .Unreachable,
            .NotSignedIn,
            .NeedsReauth,
            .Unavailable,
            .Refused(reason: .unavailable),
            .Refused(reason: .other),
        ]
        for error in errors {
            #expect(allodiaWriteFailure(error) == .unexplained)
        }
        #expect(allodiaWriteFailure(CocoaError(.fileNoSuchFile)) == .unexplained)
    }

    /// Every ending says something, except the one whose answer is somewhere else.
    ///
    /// ⚠️ **A restart the service answered with a payment page says nothing here**, because this
    /// platform may not open one: the external-purchase-link entitlement is not in place. The
    /// re-read that follows is what reports where the subscription actually stands.
    @Test func everyEndingEarnsASentenceExceptTheOneThatHasNoneToGive() {
        #expect(allodiaWriteNote(.silent, currency: "EUR") == nil)
        let sentences = [
            allodiaWriteNote(.cancelled(endDate: "2026-10-18T00:00:00Z"), currency: "EUR"),
            allodiaWriteNote(.reactivated, currency: "EUR"),
            allodiaWriteNote(.switched(change), currency: "EUR"),
            allodiaWriteNote(.failed(.notSwitchable), currency: "EUR"),
        ]
        #expect(sentences.allSatisfy { $0?.isEmpty == false })
        #expect(Set(sentences.compactMap { $0 }).count == sentences.count)
    }

    /// A cancellation names the day access runs to, formatted rather than left as the wire shape.
    @Test func aCancellationSaysWhatIsKeptAndUntilWhen() {
        let note = allodiaWriteNote(.cancelled(endDate: "2026-10-18T00:00:00Z"), currency: "EUR")
        #expect(note?.contains("2026-10-18T00:00:00Z") == false)
        #expect(note?.contains("2026") == true)
    }

    /// ⚠️ The switch answers an amount with no currency beside it, so it is priced from the
    /// subscription's own list currency. A bare number is not a price.
    @Test func aSwitchSaysWhatTheNextChargeBecomes() {
        let note = allodiaWriteNote(.switched(change), currency: "EUR")
        #expect(note?.contains("24") == true)
        // A service that sent no currency still produces a sentence rather than nothing at all.
        #expect(allodiaWriteNote(.switched(change), currency: nil)?.isEmpty == false)
    }

    /// Each refusal is a different thing to say. One sentence reused would tell somebody mid-retry
    /// that they are already cancelled.
    @Test func noTwoRefusalsSayTheSameThing() {
        let failures: [AllodiaWriteFailure] = [
            .unexplained, .alreadyCancelled, .alreadyActive, .alreadyOnInterval, .notSwitchable,
            .notFound,
        ]
        #expect(Set(failures.map(allodiaRefusalText)).count == failures.count)
    }

    private let change = AllodiaIntervalChange(
        interval: .yearly,
        amountInCents: 2499,
        nextPaymentDate: "2026-10-18T00:00:00Z"
    )

    private func subscription(
        own: AllodiaOwnSubscription? = nil,
        periodEnd: String? = "2026-10-18T00:00:00Z",
        canCancel: Bool = false,
        canResubscribe: Bool = false,
        canSwitch: Bool = false
    ) -> AllodiaSubscription {
        AllodiaSubscription(
            entitled: own != nil,
            plan: "services",
            currentPeriodEnd: periodEnd,
            own: own,
            stores: [],
            duplicateBilling: [],
            actions: AllodiaSubscriptionActions(
                canCancel: canCancel,
                canResubscribe: canResubscribe,
                canSwitchInterval: canSwitch,
                canStartCheckout: false
            ),
            prices: AllodiaPrices(monthlyInCents: 249, yearlyInCents: 2499, currency: "EUR"),
            checkoutAvailable: true
        )
    }

    private func own(
        _ status: AllodiaOwnStatus,
        interval: AllodiaPlan = .yearly
    ) -> AllodiaOwnSubscription {
        AllodiaOwnSubscription(
            status: status,
            interval: interval,
            amountInCents: 2499,
            nextPaymentDate: status == .cancelled ? nil : "2026-10-18T00:00:00Z",
            currentPeriodEnd: "2026-10-18T00:00:00Z",
            cancelledAt: status == .cancelled ? "2026-09-18T00:00:00Z" : nil
        )
    }
}
