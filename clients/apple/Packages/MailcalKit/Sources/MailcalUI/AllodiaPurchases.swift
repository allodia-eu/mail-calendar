// The seam between the App Store and the account service: one redemption pass, and the listener
// that starts one when Apple reports a transaction nobody asked for.
//
// The rules are `purchasing.md`, the contract beside the Allodia Licence. The order is the whole
// of it: **attach, then finish**, never the reverse. A transaction finished before the account
// service has attached it is money taken for nothing, because StoreKit's re-delivery is the only
// copy of it there is.
//
// This is also what makes a purchase survive the ordinary accidents. The app killed mid-flight, the
// network gone, an Ask to Buy approved three days later on a different device: in every one of them
// the transaction is still unfinished, so the next pass sees it and carries it over.

import Foundation
import MailcalBindings
import StoreKit

/// Runs redemption passes and keeps a listener on the store.
///
/// One per app. It owns no UI state: a screen asks it to buy and reads the entitlement afterwards
/// like any other.
@MainActor
final class AllodiaPurchases {
    private let app: MailcalApp
    private let store: AllodiaStoreKit
    private var listener: Task<Void, Never>?

    init(app: MailcalApp) {
        self.app = app
        self.store = AllodiaStoreKit(catalogue: app.allodiaStoreProducts(store: .apple))
    }

    deinit {
        listener?.cancel()
    }

    /// Starts watching for transactions this app did not start.
    ///
    /// **Required, not an optimisation.** A subscription renewed in the background, an Ask to Buy
    /// approved by a parent, a purchase made on another of the person's devices and a refund all
    /// arrive here and nowhere else. Started as early as the app has an account to attach them to,
    /// and Apple's own guidance is to start it at launch so nothing is missed while the app is
    /// coming up.
    func startListening() {
        guard listener == nil else { return }
        listener = Task { [weak self] in
            for await _ in Transaction.updates {
                guard let self else { return }
                // What arrived is not read here. The pass asks the store for everything it is
                // still offering, which includes this one, so there is one path rather than two
                // and no chance of them disagreeing.
                _ = try? await self.linkOutstanding()
            }
        }
    }

    /// What the App Store will sell, in the order the core says to draw it.
    ///
    /// Offers from Allodia's own checkout are added by the caller, which knows whether this
    /// platform may draw them at all; the ordering is the core's either way.
    func offers(including others: [AllodiaOffer] = []) async -> [AllodiaOffer] {
        let fromStore = (try? await store.offers()) ?? []
        return app.orderAllodiaOffers(offers: fromStore + others)
    }

    /// Buys a period and carries the purchase through to the account service.
    ///
    /// Returns once the purchase has been attached or has been left safely outstanding. A person
    /// who closes the app in between loses nothing: the transaction is unfinished, so the next
    /// launch picks it up.
    @discardableResult
    func buy(_ plan: AllodiaPlan) async throws -> AllodiaPurchaseOutcome {
        let outcome = try await store.buy(plan)
        if case .bought = outcome {
            _ = try? await linkOutstanding()
        }
        return outcome
    }

    /// One pass: hand the account service everything the store is still offering, then finish only
    /// what it says it is done with.
    ///
    /// Safe to call at any time and from anywhere; the core deduplicates and paces the retries.
    @discardableResult
    func linkOutstanding() async throws -> AllodiaPurchaseReport {
        let outstanding = await store.unfinished()
        guard !outstanding.isEmpty else { return AllodiaPurchaseReport() }
        // The pass makes network round trips and the core call blocks on them, so it goes off the
        // main thread exactly as the sign-in and sync passes do.
        let report = try await Task.detached(priority: .utility) { [app] in
            try app.linkAllodiaPurchases(purchases: outstanding)
        }.value
        // ⚠️ Only what the report names, and only now that it exists.
        await store.finish(transactionIDs: report.finish)
        return report
    }
}

extension AllodiaPurchaseReport {
    /// An empty pass, for the case where the store is offering nothing.
    fileprivate init() {
        self.init(finish: [], claimedElsewhere: [], waiting: [], anythingStuck: false)
    }
}
