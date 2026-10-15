// The App Store half of purchasing, driven against a simulated store.
//
// `Allodia.storekit` is Xcode's own StoreKit Configuration: it fakes the entire store locally, with
// no App Store Connect and no sandbox account, and `SKTestSession` buys from it without a purchase
// sheet. That is what makes these assertions possible at all, because no client draws a purchase
// screen yet.
//
// ⚠️ **What this cannot prove.** A locally simulated transaction is signed by a local test
// certificate, so Apple's App Store Server API has never heard of it and the account service will
// answer `unknown_purchase` for one. Everything up to handing the identifier over is exercised
// here; the round trip needs products in App Store Connect and a sandbox tester. That is also why
// `unknown_purchase` is retried rather than concluded from: under this configuration it is the
// ordinary answer about a purchase that is perfectly real.
//
// The prices in the configuration are the marked-up ones a store would charge, and nothing here
// asserts them: they are a fixture, and the real figures live in App Store Connect.

import MailcalBindings
import StoreKit
import StoreKitTest
import XCTest

@testable import MailcalUI

// ⚠️ **The test store is never cleared between tests.** `clearTransactions`, like a new
// `SKTestSession` (which loads the configuration into the store again), reaches StoreKit's own view
// of the transactions some time after it reaches the store, and replaces that view when it lands. A
// purchase made in between is missing from `Transaction.unfinished`, or read once and then gone,
// sometimes for good, although `allTransactions()` holds it unfinished throughout. Clearing before
// each test failed about one test in twenty that way.
//
// So the store is cleared once, before the first test, and given time to land. Each test then
// leaves the store as it found it the way an app would: it finishes the purchase it made, and
// expires the subscription so the next test can buy it again. And each test follows its own
// purchase by its id, so one an earlier test left behind cannot be mistaken for it.
final class AllodiaStoreKitTests: XCTestCase {
    nonisolated(unsafe) private static var shared: SKTestSession?
    private var session: SKTestSession!
    private var bought: [String] = []

    override func setUp() async throws {
        try await super.setUp()
        if Self.shared == nil {
            let session = try SKTestSession(configurationFileNamed: "Allodia")
            // The purchase sheet never appears, which is what lets this run without a UI.
            session.disableDialogs = true
            session.clearTransactions()
            // Not something to wait on: nothing observable says the clear has landed.
            try await Task.sleep(nanoseconds: 2_000_000_000)
            Self.shared = session
        }
        session = Self.shared
        bought = []
    }

    override func tearDown() async throws {
        for await verification in Transaction.unfinished {
            let transaction = verification.unsafePayloadValue
            if bought.contains(String(transaction.id)) { await transaction.finish() }
        }
        for product in catalogue().map(\.productId) {
            try? session.expireSubscription(productIdentifier: product)
        }
        session = nil
        try await super.tearDown()
    }

    /// Every product asked for resolves, with the store's own price string.
    ///
    /// ⚠️ What this does **not** prove is that the ids match the core's, because the catalogue
    /// below is written out here rather than read from a booted `MailcalApp`. The ids are pinned on
    /// the core's side instead, by `the_published_identifiers_are_the_ones_the_consoles_hold`, and
    /// the two lists have to be changed together. Neither side can prove the third thing, which is
    /// that App Store Connect holds the same ids.
    func testTheStoreKnowsEveryProductTheCoreAsksFor() async throws {
        let store = AllodiaStoreKit(catalogue: catalogue())

        let offers = try await store.offers()

        XCTAssertEqual(offers.count, 2, "both periods resolve")
        XCTAssertEqual(Set(offers.map(\.store)), [.apple])
        XCTAssertEqual(Set(offers.map(\.plan)), [.monthly, .yearly])
        // Drawn, never parsed: the store's own formatted string, whatever it happens to be.
        XCTAssertTrue(offers.allSatisfy { !$0.displayPrice.isEmpty })
    }

    /// A purchase the store has taken is reported until something finishes it, and what crosses is
    /// the transaction id and the store's own purchase date in **seconds**.
    func testABoughtPurchaseIsReportedWithItsIdentifierAndSecondsTimestamp() async throws {
        let store = AllodiaStoreKit(catalogue: catalogue())
        let bought = try await buy("eu.allodia.mailcal.services.yearly")

        let found = await reported(bought, by: store)

        let purchase = try XCTUnwrap(found, "StoreKit never offered the purchase")
        XCTAssertEqual(purchase.store, .apple)
        // Seconds, not milliseconds: a purchase in 1970 reads as one stuck since the epoch.
        let now = Int64(Date().timeIntervalSince1970)
        XCTAssertLessThanOrEqual(abs(now - purchase.purchasedAt), 300)
    }

    /// **The rule the whole design turns on.** Reading the outstanding set must not finish
    /// anything: until the account service has attached the purchase, StoreKit's re-delivery is the
    /// only copy of it there is.
    func testReadingTheOutstandingSetFinishesNothing() async throws {
        let store = AllodiaStoreKit(catalogue: catalogue())
        let bought = try await buy("eu.allodia.mailcal.services.monthly")
        let first = await reported(bought, by: store)
        XCTAssertNotNil(first, "the purchase is there before anything reads it twice")

        _ = await store.unfinished()
        _ = await store.unfinished()

        let stillThere = await reported(bought, by: store)
        XCTAssertNotNil(stillThere, "still offered, because nothing has attached it")
    }

    /// And finishing takes only what it was named. Anything else would settle a purchase the
    /// service has not accepted.
    func testOnlyTheNamedPurchaseIsFinished() async throws {
        let store = AllodiaStoreKit(catalogue: catalogue())
        let bought = try await buy("eu.allodia.mailcal.services.yearly")
        let found = await reported(bought, by: store)
        let purchase = try XCTUnwrap(found, "StoreKit never offered the purchase")

        await store.finish(transactionIDs: ["not-this-one"])
        let untouched = await reported(bought, by: store)
        XCTAssertNotNil(untouched, "a purchase nobody named stays outstanding")

        // `finish` looks the id up in the outstanding set, so it is named until it is gone: the
        // core names it again at every pass until the store stops offering it.
        var settled = false
        for _ in 0..<attempts where !settled {
            await store.finish(transactionIDs: [purchase.purchase])
            settled = await !store.unfinished().contains { $0.purchase == bought }
            if !settled { try? await Task.sleep(nanoseconds: 20_000_000) }
        }
        XCTAssertTrue(settled, "the named purchase is finished")
    }

    // ⚠️ There is deliberately no test here for a purchase held for Ask to Buy, because this suite
    // cannot hold one. `SKTestSession.buyProduct` completes the purchase whatever
    // `askToBuyEnabled` says, and `Product.purchase` fails with `invalidPresentationContext` in a
    // host with no window. So whether StoreKit surfaces a deferred transaction through
    // `Transaction.unfinished` is not something this suite can state.
    //
    // What it would cost if StoreKit does surface one: the purchase is handed over, the service
    // answers `unknown_purchase` because Apple has no completed transaction for it, and the ledger
    // retries until the approval lands. No money is lost, but the "not gone through" warning could
    // reach somebody who is simply waiting for a parent, which is the wrong thing to tell them.
    // `purchasing.md` carries this under its known gaps.

    /// How often a wait below asks StoreKit again, 20 ms apart: ten seconds in all.
    private let attempts = 500

    /// Buys `product` in the test store, and answers the transaction's id as the core sees it.
    private func buy(_ product: String) async throws -> String {
        let id = String(try await session.buyProduct(identifier: product).id)
        bought.append(id)
        return id
    }

    /// The purchase `id`, once StoreKit's own view offers it, or `nil` if it never does.
    private func reported(_ id: String, by store: AllodiaStoreKit) async -> AllodiaStorePurchase? {
        for _ in 0..<attempts {
            if let purchase = await store.unfinished().first(where: { $0.purchase == id }) {
                return purchase
            }
            try? await Task.sleep(nanoseconds: 20_000_000)
        }
        return nil
    }

    /// What the core is told to ask the store for. The same ids `Plan::store_product` returns, and
    /// the same ones `Allodia.storekit` publishes; all three move together.
    private func catalogue() -> [AllodiaStoreProduct] {
        [
            AllodiaStoreProduct(
                plan: .yearly,
                productId: "eu.allodia.mailcal.services.yearly",
                basePlan: nil
            ),
            AllodiaStoreProduct(
                plan: .monthly,
                productId: "eu.allodia.mailcal.services.monthly",
                basePlan: nil
            ),
        ]
    }
}
