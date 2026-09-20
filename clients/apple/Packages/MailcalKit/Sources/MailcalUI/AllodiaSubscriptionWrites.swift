// What may be done to the subscription Allodia bills directly, and what each ending is called.
//
// ⚠️ **Only Allodia's own subscription, never a store's.** One bought through a store is cancelled,
// switched and refunded at that store, so the section offers its manage page for those and none of
// this. What decides whether a button exists at all is `actions`, which the service computed with
// every biller in view; a client is in no position to work that out and does not try.
//
// ⚠️ **Apple draws three of the four writes, and never the checkout.** Starting one means sending
// somebody to a payment page outside the app, which needs Apple's external-purchase-link
// entitlement, a declared target and Apple's own disclosure sheet. None of that is in place, so on
// this platform the store's own price stands alone and buying is StoreKit's. `purchasing.md` is
// where that rule lives.
//
// SwiftUI-free and pure on purpose, like the billing decisions next door: which sentence follows
// which ending is then a test rather than something only a live subscription can show. The twins
// are `AllodiaWriteResult.cs`, `AllodiaSubscriptionModel.kt` and `allodia_subscription_facts.rs`;
// keep the wording in step.

import Foundation
import MailcalBindings

/// How one write ended.
enum AllodiaWriteResult: Equatable {
    /// The recurring charge was stopped. Access runs to the date.
    case cancelled(endDate: String)
    /// Running again on the authorisation already held: nothing to pay, nothing to re-enter.
    case reactivated
    /// The next charge moved to the other period. Nothing moved today.
    case switched(AllodiaIntervalChange)
    /// Nothing to say. The service answered a restart with a payment page rather than a
    /// reactivation, and this platform may not open one, so the re-read is what reports where the
    /// subscription actually stands.
    case silent
    /// It did not go through, and the reason decides which sentence is owed.
    case failed(AllodiaWriteFailure)
}

/// Why a write did not happen, as the sentence to put in front of the person.
///
/// ⚠️ **A code, never a message from anywhere else.** The core reports a refusal as an
/// `AllodiaSubscriptionRefusal` and `purchasing.md` asks a client to switch on it, because "you
/// have already cancelled" and "that cannot change while a charge is being retried" are different
/// things to say and only the code tells them apart. Rendering the error instead puts a generated
/// description on screen: `errorDescription` is `String(reflecting:)`, so a refusal reads
/// `Refused(reason: MailcalBindings.AllodiaSubscriptionRefusal.notSwitchable)`.
enum AllodiaWriteFailure: Equatable {
    /// Nothing more can be said: an outage, a refused token, a reason this build has never heard
    /// of. The sentence says only that nothing changed.
    case unexplained
    case alreadyCancelled
    case alreadyActive
    case alreadyOnInterval
    /// A charge is mid-retry, so the period cannot move underneath it.
    case notSwitchable
    case notFound
}

/// The sentence an error earns.
///
/// Everything that is not a refusal is `unexplained`, a service that could not be reached
/// included: nothing was learned, so there is nothing to explain, and the one thing worth saying
/// is that nothing changed.
func allodiaWriteFailure(_ error: Error) -> AllodiaWriteFailure {
    guard
        let purchase = error as? AllodiaPurchaseError,
        case let .Refused(reason) = purchase
    else { return .unexplained }
    switch reason {
    case .alreadyCancelled: return .alreadyCancelled
    case .alreadyActive: return .alreadyActive
    case .alreadyOnInterval: return .alreadyOnInterval
    case .notSwitchable: return .notSwitchable
    case .notFound: return .notFound
    // Billing not configured on this deployment, and a reason a later service invents, both leave
    // a client with nothing specific it could truthfully say.
    case .unavailable, .other: return .unexplained
    }
}

/// The period a switch would move to, which is simply the other one, or `nil` when there is
/// nothing to switch: no subscription of Allodia's own, or a service that has said this account
/// may not switch.
///
/// `actions` decides, and nothing else. Somebody the App Store is charging has a period too, and
/// switching that one through this API is exactly what the service refuses.
func allodiaSwitchTarget(_ subscription: AllodiaSubscription) -> AllodiaPlan? {
    guard subscription.actions.canSwitchInterval, let own = subscription.own else { return nil }
    switch own.interval {
    case .monthly: return .yearly
    case .yearly: return .monthly
    }
}

/// Whether to offer starting the subscription again, which is a different question from whether it
/// has been cancelled: a cancelled subscription whose period has run out is a new checkout rather
/// than a restart, and only the service knows which.
func allodiaOffersRestart(_ subscription: AllodiaSubscription) -> Bool {
    subscription.actions.canResubscribe && subscription.own?.status == .cancelled
}

/// The day both confirmations talk about: what has already been paid for, as the service sent it,
/// or the empty string when it sent none.
///
/// ⚠️ The date is the whole reassurance. Somebody cancelling wants to know they are not losing what
/// they have already paid for, and a cancellation with no date reads as "it stops now", which is
/// the one thing it does not do.
func allodiaWriteDate(_ subscription: AllodiaSubscription) -> String {
    subscription.own?.currentPeriodEnd ?? subscription.currentPeriodEnd ?? ""
}

/// Minor units and a currency as something a reader recognises.
///
/// ⚠️ A switch answers an amount with no currency beside it, so it is priced from the
/// subscription's own `prices.currency`, which is today's list currency rather than the one this
/// subscriber was charged in. The two differ only for somebody whose billing currency has since
/// changed, which the service does not currently do.
@MainActor func allodiaMinorUnits(_ minorUnits: Int64, currency: String?) -> String {
    let amount = Decimal(minorUnits) / 100
    guard let currency, !currency.isEmpty else { return "\(amount)" }
    allodiaAmountFormatter.currencyCode = currency
    return allodiaAmountFormatter.string(from: amount as NSDecimalNumber) ?? "\(amount)"
}

/// Built once: `NumberFormatter` loads ICU locale data on construction, which the date helpers next
/// door carry the same warning about.
@MainActor private let allodiaAmountFormatter: NumberFormatter = {
    let output = NumberFormatter()
    output.numberStyle = .currency
    return output
}()
