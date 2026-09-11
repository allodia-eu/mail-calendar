// Buying a subscription through Apple's App Store, the StoreKit half.
//
// The rules are `purchasing.md`, the contract beside the Allodia Licence, and the core holds every
// one of them. What lives here is the part no Rust can do: asking StoreKit for products, putting up
// the purchase sheet, collecting what the store is still offering, and finishing a transaction once
// the core says it has been granted.
//
// ⚠️ **Nothing is finished until the core says so.** StoreKit re-delivers an unfinished transaction
// at every launch, for as long as it takes, and that is the only copy of a purchase somebody paid
// for. Finishing one before the account service has granted it throws that copy away.
//
// What is handed over is the transaction's **identifier**, and nothing else. The service reads
// every fact about the subscription back from Apple itself, with a key that exists nowhere on a
// device, so there is nothing here to sign, to verify or to claim about what was bought.

import Foundation
import MailcalBindings
import StoreKit

/// A failure in the store half of the flow. A cancelled sheet is not one of these: it is an
/// outcome, and it says nothing.
enum AllodiaPurchaseFailure: LocalizedError {
    case noSuchProduct
    case storeUnavailable(String)

    var errorDescription: String? {
        switch self {
        case .noSuchProduct:
            return "This subscription is not available from the App Store right now."
        case let .storeUnavailable(reason):
            return reason
        }
    }
}

/// How one trip through the purchase sheet ended.
enum AllodiaPurchaseOutcome {
    /// The store took the money. The purchase still has to be attached before anything is
    /// finished.
    case bought(AllodiaStorePurchase)
    /// The sheet was dismissed. Not a failure, and nothing is said about it.
    case cancelled
    /// Apple is waiting on somebody else: Ask to Buy, or a payment needing approval. The
    /// transaction arrives later through `Transaction.updates`, so there is nothing to do now
    /// beyond telling the person it is pending.
    case awaitingApproval
}

/// The App Store, as this app uses it.
///
/// An actor because StoreKit is reached from a launch pass, from a purchase sheet and from the
/// updates stream, and the set of products fetched once should not be fetched three times.
actor AllodiaStoreKit {
    /// What the core says to ask the store for, one entry per period. Read once: the identifiers
    /// are constants on both sides.
    private let catalogue: [AllodiaStoreProduct]
    private var products: [String: Product] = [:]

    init(catalogue: [AllodiaStoreProduct]) {
        self.catalogue = catalogue
    }

    /// Everything the App Store will sell here, with the store's own localised price.
    ///
    /// The price string is **drawn, never parsed**: it is already localised, the App Store's
    /// commission is already in it, and Apple requires the store's own formatted price be shown
    /// rather than one an app assembled.
    ///
    /// A product the store does not know about is left out rather than raised. A storefront can
    /// legitimately lack one, and a person should see the plans that are available rather than an
    /// error about the one that is not.
    func offers() async throws -> [AllodiaOffer] {
        try await loadProducts()
        return catalogue.compactMap { wanted in
            guard let product = products[wanted.productId] else { return nil }
            return AllodiaOffer(
                plan: wanted.plan,
                store: .apple,
                displayPrice: product.displayPrice
            )
        }
    }

    /// Puts up the purchase sheet for one period.
    ///
    /// What comes back is **not** a granted subscription. The core attaches it to the account
    /// first, and only what the core names is finished.
    func buy(_ plan: AllodiaPlan) async throws -> AllodiaPurchaseOutcome {
        try await loadProducts()
        guard
            let wanted = catalogue.first(where: { $0.plan == plan }),
            let product = products[wanted.productId]
        else {
            throw AllodiaPurchaseFailure.noSuchProduct
        }

        switch try await product.purchase() {
        case let .success(verification):
            return .bought(purchase(from: verification))
        case .userCancelled:
            return .cancelled
        case .pending:
            return .awaitingApproval
        @unknown default:
            // A result this build does not know is not a purchase it may act on. Whatever it
            // turns out to be, StoreKit will offer the transaction again if one was made.
            return .cancelled
        }
    }

    /// Every purchase the store is still offering, which is every one nothing has finished.
    ///
    /// Called at launch and after a purchase, and handed to the core whole: the store is the
    /// authority on what is outstanding, so a pass is told the set rather than the difference.
    func unfinished() async -> [AllodiaStorePurchase] {
        var outstanding: [AllodiaStorePurchase] = []
        for await verification in Transaction.unfinished {
            outstanding.append(purchase(from: verification))
        }
        return outstanding
    }

    /// Finishes the transactions the core has had granted, and only those.
    ///
    /// Matched by id against what the store is offering rather than finished by a handle the
    /// caller kept, so a transaction that arrived on the updates stream between the pass and this
    /// call is still the one that gets finished.
    func finish(transactionIDs: [String]) async {
        let wanted = Set(transactionIDs)
        guard !wanted.isEmpty else { return }
        for await verification in Transaction.unfinished {
            let transaction = verification.unsafePayloadValue
            guard wanted.contains(String(transaction.id)) else { continue }
            await transaction.finish()
        }
    }

    /// Loads the products once per process.
    ///
    /// A storefront change or a network failure leaves this empty, which reads as "no offers" and
    /// draws nothing rather than an error, the same posture the entitlement rules take.
    private func loadProducts() async throws {
        guard products.isEmpty else { return }
        do {
            let fetched = try await Product.products(for: catalogue.map(\.productId))
            products = Dictionary(uniqueKeysWithValues: fetched.map { ($0.id, $0) })
        } catch {
            throw AllodiaPurchaseFailure.storeUnavailable(error.localizedDescription)
        }
    }

    /// What the core carries to the account service: which store, which transaction, and when.
    ///
    /// **Whether StoreKit could verify it locally is deliberately not consulted.** The service
    /// reads the transaction back from Apple and is the only party that can; a device that
    /// discarded an unverified one would be throwing away a real purchase over its own clock or a
    /// certificate it could not fetch, and one that trusted a verified one would be deciding
    /// something it has no standing to decide.
    private func purchase(
        from verification: VerificationResult<Transaction>
    ) -> AllodiaStorePurchase {
        let transaction = verification.unsafePayloadValue
        return AllodiaStorePurchase(
            store: .apple,
            purchase: String(transaction.id),
            purchasedAt: Int64(transaction.purchaseDate.timeIntervalSince1970)
        )
    }
}
