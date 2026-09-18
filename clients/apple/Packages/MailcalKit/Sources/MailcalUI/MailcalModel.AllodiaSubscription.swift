// The subscription side of the Allodia account: the one read that answers the whole section, and
// the purchase that starts one.
//
// The rules are `purchasing.md`, the contract beside the Allodia Licence, and the core holds every
// one of them. What is here is the hop off the main thread that each blocking core call takes, and
// the translation of a core answer into the few things a screen can be in.
//
// ⚠️ **None of this gates a capability.** `allodiaEntitlement` does, locally and without a network
// call. This read is what the screen *says*, so a service that cannot be reached costs somebody a
// sentence, never access they have paid for.

import Foundation
import MailcalBindings

/// What the subscription section has to draw.
enum AllodiaSubscriptionState: Equatable {
    /// The read is in flight. Distinct from `unavailable`: nothing has been learned yet, and a
    /// spinner and a failure are different things to put in front of somebody.
    case checking
    /// The read did not come back, or this deployment has no billing configured.
    case unavailable
    /// The read could not be made at all, because this device's sign-in predates the permission it
    /// needs. An offer rather than an error: they are signed in, one thing is asleep, and the
    /// ordinary sign-in asks for the full current scope set. Kept apart from `unavailable` because
    /// the remedies differ: waiting fixes an outage and never fixes this.
    case needsReauth
    /// The service answered.
    case loaded(AllodiaSubscriptionView)
}

/// The answer, with the parts a screen needs alongside it.
struct AllodiaSubscriptionView: Equatable {
    let subscription: AllodiaSubscription
    /// What can be bought here, in the core's order. Empty while something is already being
    /// charged: the service refuses a second subscription for one plan, so offering one would be
    /// drawing a button whose only outcome is a refusal.
    let offers: [AllodiaOffer]
    /// Whether some purchase has been waiting long enough to be worth saying so. Money was taken
    /// and nothing has been granted, and a screen that draws nothing here leaves the person with
    /// no way to find that out.
    let anythingStuck: Bool
}

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

/// What a read that did not come back is allowed to put on screen.
///
/// ⚠️ **The core's typed answer decides, never the failure's text.** A sign-in that predates the
/// permission this read needs is an offer with a remedy, and drawing it as an outage tells somebody
/// to wait for something that will never arrive. The sync card next door reads the same answer for
/// the same reason, and everything else says nothing about the sign-in at all.
///
/// Pure, so the choice is unit-testable: the pane around it is not, needing a live `MailcalApp`.
func allodiaReadFailure(_ health: AllodiaGrantHealth) -> AllodiaSubscriptionState {
    health == .needsReauth ? .needsReauth : .unavailable
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

    /// Everything the subscription section draws, in one pass.
    ///
    /// The redemption pass runs first, so a purchase made on another device, or one this device
    /// could not attach last time, is attached before the state it produces is read back.
    func allodiaSubscriptionState() async -> AllodiaSubscriptionState {
        guard let app else { return .unavailable }
        let stuck = await runAllodiaRedemption()
        // The read is a network round trip and the core call blocks on it, so it goes off the main
        // thread exactly as the sign-in and sync passes do.
        let answer = await Task.detached(priority: .userInitiated) {
            try? app.allodiaSubscription()
        }.value
        guard let answer else { return allodiaReadFailure(app.allodiaGrantHealth()) }
        // Offers cost a StoreKit round trip, so they are fetched only when they can be drawn.
        let offers = answer.entitled ? [] : await allodiaPurchases?.offers() ?? []
        return .loaded(
            AllodiaSubscriptionView(subscription: answer, offers: offers, anythingStuck: stuck)
        )
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

    /// One redemption pass, reporting only whether something has been stuck long enough to say so.
    ///
    /// A pass that could not reach the service is not a failure here: the purchase stays
    /// outstanding and the next pass tries again, which is the whole of what keeps it safe.
    private func runAllodiaRedemption() async -> Bool {
        guard let purchases = allodiaPurchases else { return false }
        return (try? await purchases.linkOutstanding())?.anythingStuck ?? false
    }
}
