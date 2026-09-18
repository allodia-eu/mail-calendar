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
    ///
    /// ⚠️ **Every shape here is one the account service actually sends**, and the offset form is a
    /// regression test: it was parsed by a helper that demands a `Z`, so a real renewal date
    /// reached a Dutch screen as `2026-09-18` rather than `18 september 2026`. A formatted answer
    /// is one that no longer looks like the wire, which is what these assert.
    @Test func aPeriodEndIsFormattedWhateverShapeItArrivesIn() {
        for raw in [
            "2026-10-18T09:30:00Z",
            "2026-10-18T09:30:00+00:00",
            "2026-10-18T11:30:00+02:00",
            "2026-10-18T09:30:00.123456Z",
            "2026-10-18",
        ] {
            let shown = allodiaDate(raw)
            #expect(shown != nil, "\(raw) produced nothing")
            #expect(shown != raw, "\(raw) was drawn as the wire rather than formatted")
            #expect(shown?.contains("2026") == true, "\(raw) lost its year")
        }
    }

    /// Nothing to say is said as nothing, rather than as an empty line where a date belongs.
    @Test func anAbsentPeriodEndIsNoLineAtAll() {
        #expect(allodiaDate("") == nil)
    }

    /// A **time** this build cannot read still yields a localised day, because the day is the only
    /// part this screen draws: the leading ten characters parse even when the rest does not.
    @Test func anUnreadableTimeStillYieldsALocalisedDay() {
        let shown = allodiaDate("2026-10-18 09:30 CET")
        #expect(shown != nil)
        #expect(shown != "2026-10-18", "the day parsed, so it should have been formatted")
        #expect(shown?.contains("2026") == true)
    }

    /// Something that is not a date at all is drawn as what arrived rather than as a guess. It is
    /// wrong either way; showing the wire at least says so.
    @Test func somethingThatIsNoDateIsNotInvented() {
        #expect(allodiaDate("no-such-date") == "no-such-da")
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

    /// ⚠️ A store that has stopped charging is not a biller, whatever order the service listed it
    /// in.
    ///
    /// Observed on an Android device: the account had bought through the App Store hours earlier,
    /// that sandbox subscription had since expired, and a purchase through Google Play was then
    /// told "Billed by Apple". The service was right both times; the reading of it was not. This
    /// screen had the same gap.
    @Test func aStoreThatHasStoppedChargingIsNotWhoIsBillingYou() {
        let subscription = subscription(stores: [
            store(source: .apple, status: .expired, autoRenewing: false),
            store(source: .google, status: .active, autoRenewing: true),
        ])
        #expect(allodiaBillers(of: subscription) == [.google])
    }

    /// ⚠️ Two subscriptions at one store get one way in, not two.
    ///
    /// Resubscribing is what produces the pair: the lapsed one is still inside the period it was
    /// paid for, so both are manageable. Found on an Android device, which drew "Manage at Google
    /// Play" twice, both opening the same page. A store's subscription page is the store's, not the
    /// subscription's.
    @Test func twoSubscriptionsAtOneStoreGetOneWayIn() {
        let both = subscription(stores: [
            store(source: .google, status: .cancelled, autoRenewing: false),
            store(source: .google, status: .active, autoRenewing: true),
        ])
        #expect(allodiaManageableStores(of: both).count == 1)
        #expect(allodiaBillers(of: both) == [.google])

        // Two different stores keep two of each: it is one per store, not one in total.
        let apart = subscription(stores: [
            store(source: .apple, status: .active, autoRenewing: true),
            store(source: .google, status: .active, autoRenewing: true),
        ])
        #expect(allodiaManageableStores(of: apart).count == 2)
        #expect(allodiaBillers(of: apart) == [.apple, .google])
    }

    /// ⚠️ "Who is billing you" and "is there anything to do at the store" are different questions,
    /// and they part company on the two states where somebody most needs the answer.
    ///
    /// On hold and paused grant nothing, so neither may say it is billing you; both are fixed only
    /// at the store, so both keep the way there. Expired and revoked are the other way about:
    /// nothing to manage, and offering the route walks somebody into the store's own resubscribe
    /// button while another source is already charging them.
    @Test func aLapsedStoreOffersNoWayInButAHeldOneDoes() {
        for status in [AllodiaStoreStatus.onHold, .paused] {
            #expect(!allodiaStoreIsBilling(status))
            #expect(allodiaStoreCanBeManaged(status))
        }
        for status in [AllodiaStoreStatus.expired, .revoked] {
            #expect(!allodiaStoreIsBilling(status))
            #expect(!allodiaStoreCanBeManaged(status))
        }
        for status in [AllodiaStoreStatus.active, .grace, .cancelled] {
            #expect(allodiaStoreIsBilling(status))
            #expect(allodiaStoreCanBeManaged(status))
        }
    }

    /// Cancelled is still being billed in the only sense that matters here: the period was paid
    /// for and is still running. Revoked, on hold and paused grant nothing and name nobody.
    @Test func aCancelledSubscriptionStillNamesItsStoreAndARevokedOneDoesNot() {
        #expect(allodiaStoreIsBilling(.active))
        #expect(allodiaStoreIsBilling(.grace))
        #expect(allodiaStoreIsBilling(.cancelled))
        #expect(!allodiaStoreIsBilling(.revoked))
        #expect(!allodiaStoreIsBilling(.onHold))
        #expect(!allodiaStoreIsBilling(.paused))
        #expect(!allodiaStoreIsBilling(.expired))
        #expect(!allodiaStoreIsBilling(.unknown(label: "something new")))
    }

    /// A checkout that was started and never paid is not somebody who is being charged.
    @Test func aCheckoutAwaitingItsFirstPaymentNamesNobody() {
        #expect(allodiaBillers(of: subscription(own: own(status: .pendingFirstPayment, next: nil))).isEmpty)
    }

    /// ⚠️ A sign-in too old to carry the permission this read needs is an offer, not an outage.
    ///
    /// The two are indistinguishable from the failure alone, and their remedies are opposites:
    /// waiting fixes an outage and never fixes this. Observed on an Android device whose grant
    /// predated the permission and which was told its subscription could not be checked "right
    /// now"; this screen had the same gap.
    @Test func aSignInTooOldToReadTheSubscriptionOffersAFreshOne() {
        #expect(allodiaReadFailure(.needsReauth) == .needsReauth)
        #expect(allodiaReadFailure(.ok) == .unavailable)
        #expect(allodiaReadFailure(.signedOut) == .unavailable)
    }

    private func store(
        source: AllodiaStore = .apple,
        status: AllodiaStoreStatus,
        autoRenewing: Bool
    ) -> AllodiaStoreSubscription {
        AllodiaStoreSubscription(
            source: source,
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
