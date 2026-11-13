// The subscription side of the Allodia account. The core reads it and runs every write to
// Allodia's own subscription (`allodiaSubscriptionCard`, `AllodiaIntent`), each returning at once;
// what is here is sending those, and the App Store purchase, which stays this client's because no
// Rust can call StoreKit.
//
// ⚠️ **None of this gates a capability.** `allodiaEntitlement` does, locally and without a network
// call. The card is what the screen *says*, so a service that cannot be reached costs somebody a
// sentence, never access they have paid for.

import Foundation
import MailcalBindings

/// How one trip through the purchase sheet ended, as a screen sees it.
enum AllodiaBuyOutcome {
    /// The store took the money. Whether it has been granted yet is the next read's answer, not
    /// this one's.
    case bought
    /// The sheet was dismissed. Not a failure, and nothing is said about it.
    case cancelled
    /// Apple is waiting on somebody else, Ask to Buy being the usual one. The transaction arrives
    /// later on the updates stream, so there is nothing to do now but say it is pending.
    case awaitingApproval
    case failed(String)
}

/// What a redemption pass owes somebody a sentence about. The ids are not carried: a person
/// cannot act on a store transaction id.
struct AllodiaPassNotes: Equatable {
    /// Some purchase has waited long enough to be worth saying so: money was taken and nothing has
    /// been granted.
    var stuck = false
    /// The pass dropped a purchase a **different** Allodia account already owns, so nothing was
    /// granted here and nothing will be.
    var claimedElsewhere = false
}

extension MailboxModel {
    /// Attaches the App Store side and starts its listener.
    ///
    /// A build carrying no Allodia registration has no account to attach a purchase to and no
    /// screen that offers one, so it opens no connection to StoreKit either: absent, rather than
    /// present and unreachable, which is the posture the whole category already takes.
    func startAllodiaPurchases(_ app: MailcalApp) {
        guard allodiaSignInAvailable() else { return }
        let purchases = AllodiaPurchases(app: app)
        allodiaPurchases = purchases
        purchases.startListening()
    }

    /// Asks the core to read the subscription again. A read already running answers for it.
    func refreshAllodiaSubscription() {
        app?.dispatch(intent: .allodia(intent: .refreshSubscription))
    }

    /// Stops the recurring charge on Allodia's own subscription, once the person confirmed.
    func cancelAllodiaSubscription() {
        app?.dispatch(intent: .allodia(intent: .cancelSubscription))
    }

    /// Moves Allodia's own subscription to `plan`, once the person confirmed.
    func switchAllodiaInterval(to plan: AllodiaPlan) {
        app?.dispatch(intent: .allodia(intent: .switchInterval(plan: plan)))
    }

    /// Starts a cancelled subscription of Allodia's own again.
    ///
    /// ⚠️ **A payment page the core hands back is never opened here**: that would be the external
    /// purchase link this platform may not draw. The read that follows says where the
    /// subscription stands.
    func resubscribeToAllodia() {
        app?.dispatch(intent: .allodia(intent: .resubscribe))
    }

    /// Carries what the store still offers to the account service, then has the subscription read
    /// again, so a purchase made on another device or one this device could not attach last time
    /// is in the answer.
    ///
    /// A pass that could not reach the service is not a failure here: the purchase stays
    /// outstanding and the next pass tries again, which is the whole of what keeps it safe.
    func linkAllodiaPurchasesThenRefresh() async -> AllodiaPassNotes {
        var notes = AllodiaPassNotes()
        if let purchases = allodiaPurchases,
           let report = try? await purchases.linkOutstanding() {
            notes = AllodiaPassNotes(
                stuck: report.anythingStuck, claimedElsewhere: !report.claimedElsewhere.isEmpty)
        }
        refreshAllodiaSubscription()
        return notes
    }

    /// Buys a period through the App Store and carries it to the account service.
    ///
    /// Returns once the purchase has been attached or has been left safely outstanding. Somebody
    /// who closes the app in between loses nothing: the transaction is unfinished, so the next
    /// launch picks it up.
    func buyAllodiaSubscription(_ plan: AllodiaPlan) async -> AllodiaBuyOutcome {
        guard let purchases = allodiaPurchases else {
            return .failed(L10n.settings_allodia_browser_failed())
        }
        do {
            // ⚠️ **Every ending is logged, the quiet ones most of all.** A dismissed sheet says
            // nothing to the person, deliberately, and StoreKit reports a sign-in that failed
            // against the sandbox as the same `userCancelled` a dismissal gives. So a purchase
            // that never had a chance and one somebody thought better of look identical on screen,
            // and the log is the only place they can be told apart.
            switch try await purchases.buy(plan) {
            case .bought:
                logAppleLifecycle("allodia: store took a \(plan) purchase")
                return .bought
            case .cancelled:
                logAppleLifecycle("allodia: a \(plan) purchase ended without one")
                return .cancelled
            case .awaitingApproval:
                logAppleLifecycle("allodia: a \(plan) purchase is waiting for approval")
                return .awaitingApproval
            }
        } catch {
            // The store's messages name products and storefronts, never an address or a secret, so
            // showing and logging one stays inside the never-log-content rule.
            logAppleLifecycle("allodia: purchase did not complete (\(error))")
            return .failed("\(error)")
        }
    }
}
