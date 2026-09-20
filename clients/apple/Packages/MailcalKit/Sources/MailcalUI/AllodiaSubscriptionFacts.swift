// What the subscription section says about who is charging, and what it may offer a way into.
//
// Pure and SwiftUI-free on purpose: every one of these is a sentence somebody reads about their own
// money, so each is decided here and asserted in `AllodiaSubscriptionSectionTests`, rather than
// being visible only on a live paid account. The twins are `AllodiaSubscriptionCard.cs`,
// `AllodiaSubscriptionModel.kt` and `allodia_subscription_facts.rs`.

import MailcalBindings

extension AllodiaBiller {
    /// What this biller is called on screen.
    ///
    /// **Allodia, never the payment processor behind it.** Naming a processor somebody has never
    /// heard of, inside a message about being charged twice, is how a correct warning reads as a
    /// scam. The bare company name is right here, where the sentence is about who is taking the
    /// money rather than about the app.
    var displayName: String {
        switch self {
        case .allodia: return "Allodia"
        case .apple: return "Apple"
        case .google: return "Google Play"
        // A biller this build cannot name still has to appear, or a "you are being charged twice"
        // warning would name only one of the two.
        case let .unknown(label): return label
        }
    }
}

extension AllodiaPlan {
    /// The button that buys this period, priced.
    ///
    /// **The period is on the button, not under it.** It is the thing being chosen, so a row of
    /// buttons that all read "Subscribe" makes the reader pair each one with a line of small print
    /// to find out what it does. The renewal terms stay underneath, because they are the same
    /// sentence for both and belong to the commitment rather than to the choice.
    func buyLabel(price: String) -> String {
        switch self {
        case .yearly: return L10n.settings_subscription_buy_yearly(price: price)
        case .monthly: return L10n.settings_subscription_buy_monthly(price: price)
        }
    }
}

/// Everyone charging for this account right now, in a stable order: Allodia's own billing first,
/// then each store in the order the service listed them.
///
/// ⚠️ **"Right now" is the whole of it, and reading every store the service listed was a bug.** The
/// list keeps a subscription after it ends, so an account that bought at one store and later at
/// another carries both, and naming the first named the dead one: an Android device billed by
/// Google Play was told it was billed by Apple, whose sandbox subscription had run out hours
/// earlier.
///
/// Whether a store still has something to manage is a different question, answered by
/// `allodiaStoreCanBeManaged`, and the two part company on the states that matter most.
///
/// Pure, so the sentence a screen says about being charged twice is unit-testable.
func allodiaBillers(of subscription: AllodiaSubscription) -> [AllodiaBiller] {
    var billers: [AllodiaBiller] = []
    if let own = subscription.own, own.status != .pendingFirstPayment {
        billers.append(.allodia)
    }
    // Distinct for the same reason the manage buttons are: two subscriptions at one store is an
    // ordinary shape, and naming that store twice would say somebody is charged twice by it.
    var billing: Set<AllodiaStore> = []
    billers.append(contentsOf: subscription.stores
        .filter { allodiaStoreIsBilling($0.status) && billing.insert($0.source).inserted }
        .map { store in
            switch store.source {
            case .apple: return .apple
            case .google: return .google
            // The service never reports its own billing as a store, so this arm is unreachable
            // rather than meaningful. Saying "Allodia" is still the right answer if it is reached.
            case .allodia: return .allodia
            }
        })
    return billers
}

/// Whether a store subscription is one somebody is still on: renewing, being retried, or cancelled
/// and running out the period already paid for.
///
/// The rest grant nothing, and `AllodiaStoreStatus` is deliberately read arm by arm rather than by
/// exclusion: a status this build does not know is **never read as permission**, which is the rule
/// the core states about the same enum.
func allodiaStoreIsBilling(_ status: AllodiaStoreStatus) -> Bool {
    switch status {
    case .active, .grace, .cancelled: return true
    case .onHold, .paused, .expired, .revoked: return false
    case .unknown: return false
    }
}

/// The stores worth offering a way into, one entry per store rather than per subscription.
///
/// ⚠️ **An account can carry more than one subscription at the same store**, and it does the moment
/// somebody resubscribes: the lapsed one is still inside the period it was paid for, so both are
/// manageable and both were drawn, as two identical buttons opening the same page. A store's
/// subscription page is the store's, not the subscription's, so one button is the whole of what
/// there is to offer.
func allodiaManageableStores(of subscription: AllodiaSubscription) -> [AllodiaStoreSubscription] {
    var seen: Set<AllodiaStore> = []
    return subscription.stores
        .filter { allodiaStoreCanBeManaged($0.status) }
        .filter { seen.insert($0.source).inserted }
}

/// Whether the store still has something for this person to do about this subscription.
///
/// ⚠️ **Not the same question as who is billing them, and the two answers differ on exactly the
/// states somebody needs most.** On hold and paused grant nothing, so neither may claim the "billed
/// by" line, and both are fixed only at the store: a card that failed is replaced there and a pause
/// is lifted there. Dropping their button strands the person it matters to.
///
/// Expired and revoked are the other way about. Nothing is left to manage, and offering the route
/// anyway walks somebody into the store's own resubscribe button while another source is already
/// charging them, which is the duplicate billing this contract warns about rather than causes.
func allodiaStoreCanBeManaged(_ status: AllodiaStoreStatus) -> Bool {
    switch status {
    case .active, .grace, .cancelled, .onHold, .paused: return true
    case .expired, .revoked: return false
    // A status this build does not know names no action it could offer.
    case .unknown: return false
    }
}

/// Whether anything will charge again, which decides between "renews on" and "runs until".
///
/// A store subscription says so itself; Allodia's own says so by still having a next payment.
/// Pure, and deliberately not a reading of `entitled`, which answers a different question.
func allodiaWillRenew(_ subscription: AllodiaSubscription) -> Bool {
    if subscription.stores.contains(where: \.autoRenewing) { return true }
    guard let own = subscription.own else { return false }
    return own.status == .active && own.nextPaymentDate != nil
}

/// Whether a charge has failed and is being retried. **Access continues**, so this is never drawn
/// as a lapse.
func allodiaInGrace(_ subscription: AllodiaSubscription) -> Bool {
    subscription.stores.contains { $0.status == .grace } || subscription.own?.status == .pastDue
}
