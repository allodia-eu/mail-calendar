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

final class AllodiaStoreKitTests: XCTestCase {
    private var session: SKTestSession!

    override func setUp() async throws {
        try await super.setUp()
        session = try SKTestSession(configurationFileNamed: "Allodia")
        session.resetToDefaultState()
        session.clearTransactions()
        // The purchase sheet never appears, which is what lets this run without a UI.
        session.disableDialogs = true
    }

    override func tearDown() async throws {
        // Finish whatever this test left outstanding, **then** clear the session. Most of these
        // tests deliberately leave a purchase unfinished, which is the whole point of them, and
        // `clearTransactions` alone does not take it out of StoreKit's own queue: the next test
        // then starts with somebody else's purchase already waiting and fails for a reason that
        // has nothing to do with what it asserts. The `before` guard below is what catches it if
        // this ever stops working.
        for await verification in Transaction.unfinished {
            await verification.unsafePayloadValue.finish()
        }
        session.clearTransactions()
        session = nil
        try await super.tearDown()
    }

    /// The catalogue the core hands out has to name products the store actually knows. A typo here
    /// is invisible in every unit test: StoreKit answers an empty list rather than an error, so the
    /// screen would simply have nothing on it.
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
        try await session.buyProduct(identifier: "eu.allodia.mailcal.services.yearly")

        let outstanding = await store.unfinished()

        XCTAssertEqual(outstanding.count, 1)
        let purchase = try XCTUnwrap(outstanding.first)
        XCTAssertEqual(purchase.store, .apple)
        XCTAssertFalse(purchase.purchase.isEmpty)
        // Seconds, not milliseconds: a purchase in 1970 reads as one stuck since the epoch.
        let now = Int64(Date().timeIntervalSince1970)
        XCTAssertLessThanOrEqual(abs(now - purchase.purchasedAt), 300)
    }

    /// **The rule the whole design turns on.** Reading the outstanding set must not finish
    /// anything: until the account service has attached the purchase, StoreKit's re-delivery is the
    /// only copy of it there is.
    func testReadingTheOutstandingSetFinishesNothing() async throws {
        let store = AllodiaStoreKit(catalogue: catalogue())
        try await session.buyProduct(identifier: "eu.allodia.mailcal.services.monthly")

        _ = await store.unfinished()
        _ = await store.unfinished()

        let stillThere = await store.unfinished()
        XCTAssertEqual(stillThere.count, 1, "still offered, because nothing has attached it")
    }

    /// And finishing takes only what it was named. Anything else would settle a purchase the
    /// service has not accepted.
    func testOnlyTheNamedPurchaseIsFinished() async throws {
        let store = AllodiaStoreKit(catalogue: catalogue())
        try await session.buyProduct(identifier: "eu.allodia.mailcal.services.yearly")
        let outstanding = await store.unfinished()
        let purchase = try XCTUnwrap(outstanding.first)

        await store.finish(transactionIDs: ["not-this-one"])
        let untouched = await store.unfinished()
        XCTAssertEqual(untouched.count, 1, "a purchase nobody named stays outstanding")

        await store.finish(transactionIDs: [purchase.purchase])
        let settled = await store.unfinished()
        XCTAssertTrue(settled.isEmpty)
    }

    /// A purchase somebody else has to approve arrives later, through the updates stream, and is
    /// **not** outstanding in the meantime: nothing has been bought yet, and treating it as bought
    /// would attach a subscription nobody has paid for.
    func testAPurchaseAwaitingApprovalIsNotYetOutstanding() async throws {
        session.askToBuyEnabled = true
        let store = AllodiaStoreKit(catalogue: catalogue())
        let before = await store.unfinished()
        XCTAssertTrue(before.isEmpty, "the session starts with nothing outstanding")

        // Held for approval, so this either throws or returns a transaction in a pending state.
        // Which of the two is Apple's business; what matters is what the store reports afterwards.
        _ = try? await session.buyProduct(identifier: "eu.allodia.mailcal.services.monthly")

        let outstanding = await store.unfinished()
        XCTAssertTrue(outstanding.isEmpty, "held for approval, so there is nothing to attach")
    }

    /// What the core is told to ask the store for, which is the one place the ids are written.
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
