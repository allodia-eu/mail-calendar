// What the subscription section says about a subscription, pinned as policy rather than as pixels.
//
// Three of these encode a rule `purchasing.md` states and a screen could plausibly get backwards:
// a failed charge that is still being retried is **not** a lapse, a subscription nobody will be
// charged for again runs out rather than renews, and every biller charging for one account has to
// appear, or a warning about paying twice names one of the two. The drawing itself stays
// hand-verified: it is a Settings pane with no state a headless test can reach.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@MainActor
struct AllodiaSubscriptionSectionTests {
    /// Access continues while a store retries a failed charge, so the section may say a payment
    /// failed and may not say the subscription has ended.
    @Test func aRetriedChargeIsNotALapse() {
        let subscription = subscription(stores: [store(status: .grace, autoRenewing: true)])
        #expect(allodiaInGrace(subscription))
        #expect(allodiaWillRenew(subscription))
    }

    /// Auto-renewal off means the date is the end of what was paid for, not a renewal date.
    @Test func aCancelledStoreSubscriptionRunsOutRatherThanRenews() {
        let subscription = subscription(stores: [store(status: .cancelled, autoRenewing: false)])
        #expect(!allodiaWillRenew(subscription))
        #expect(!allodiaInGrace(subscription))
    }

    /// Allodia's own billing renews on the strength of a next payment, and a cancelled one has
    /// none, whatever date it still carries.
    @Test func ownBillingRenewsOnlyWhileAPaymentIsStillComing() {
        #expect(allodiaWillRenew(subscription(own: own(status: .active, next: "2026-10-18T00:00:00Z"))))
        #expect(!allodiaWillRenew(subscription(own: own(status: .active, next: nil))))
        #expect(!allodiaWillRenew(subscription(own: own(status: .cancelled, next: nil))))
    }

    /// A retry at Allodia's own billing is the same non-lapse a store's grace period is.
    @Test func ownBillingPastDueIsAlsoNotALapse() {
        #expect(allodiaInGrace(subscription(own: own(status: .pastDue, next: nil))))
    }

    /// Somebody paying Allodia and Apple at once has to see both named. Reporting one would leave
    /// them cancelling the wrong subscription, or none.
    @Test func everyBillerChargingForOneAccountIsNamed() {
        let subscription = subscription(
            own: own(status: .active, next: "2026-10-18T00:00:00Z"),
            stores: [store(status: .active, autoRenewing: true)]
        )
        #expect(allodiaBillers(of: subscription) == [.allodia, .apple])
    }

    /// Nobody charging is the ordinary answer for a free account, and it is an empty list rather
    /// than a biller standing in for one.
    @Test func anAccountNobodyChargesNamesNoBiller() {
        #expect(allodiaBillers(of: subscription()).isEmpty)
    }

    /// A biller this build cannot name still appears, or the "charged twice" warning would name
    /// only the half it recognises.
    @Test func anUnknownBillerStillHasAName() {
        #expect(AllodiaBiller.unknown(label: "Paddle").displayName == "Paddle")
        #expect(AllodiaBiller.allodia.displayName == "Allodia")
    }

    /// A date is shown to the day: a renewal is what a bank statement will agree with, and an hour
    /// invites a comparison with a clock that means nothing here.
    @Test func aPeriodEndIsShownToTheDay() {
        #expect(allodiaDate("2026-10-18T09:30:00Z")?.contains("2026") == true)
        // A bare date from the service is already the answer, and is not dropped for lacking a
        // time.
        #expect(allodiaDate("2026-10-18") == "2026-10-18")
        #expect(allodiaDate("") == nil)
    }

    // MARK: builders

    private func subscription(
        own: AllodiaOwnSubscription? = nil,
        stores: [AllodiaStoreSubscription] = []
    ) -> AllodiaSubscription {
        AllodiaSubscription(
            entitled: own != nil || !stores.isEmpty,
            plan: "services",
            currentPeriodEnd: "2026-10-18T00:00:00Z",
            own: own,
            stores: stores,
            duplicateBilling: [],
            actions: AllodiaSubscriptionActions(
                canCancel: false,
                canResubscribe: false,
                canSwitchInterval: false,
                canStartCheckout: own == nil && stores.isEmpty
            ),
            prices: AllodiaPrices(monthlyInCents: 249, yearlyInCents: 2499, currency: "EUR"),
            checkoutAvailable: true
        )
    }

    private func own(status: AllodiaOwnStatus, next: String?) -> AllodiaOwnSubscription {
        AllodiaOwnSubscription(
            status: status,
            interval: .yearly,
            amountInCents: 2499,
            nextPaymentDate: next,
            currentPeriodEnd: "2026-10-18T00:00:00Z",
            cancelledAt: nil
        )
    }

    private func store(
        status: AllodiaStoreStatus,
        autoRenewing: Bool
    ) -> AllodiaStoreSubscription {
        AllodiaStoreSubscription(
            source: .apple,
            status: status,
            interval: .yearly,
            productId: "eu.allodia.mailcal.services.yearly",
            priceInCents: 2499,
            currency: "EUR",
            currentPeriodEnd: "2026-10-18T00:00:00Z",
            autoRenewing: autoRenewing,
            environment: "sandbox",
            manageUrl: "https://apps.apple.com/account/subscriptions"
        )
    }
}
