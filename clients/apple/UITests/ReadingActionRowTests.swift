// The action row above an open message (`docs/reading-actions.md`).
//
// The contract fixes an order, and an order is a fact about the screen: reply, reply-all and
// forward at the start, archive and delete at the end, the overflow button last of all. Nothing
// below the UI can observe it, which is why the rule was until now held only by the code that
// draws it.

import XCTest

@MainActor
final class ReadingActionRowTests: XCTestCase {
    /// The row's order, measured left to right.
    ///
    /// Which of archive and delete comes first is each platform's own, so the assertion is on the
    /// three groups and on the overflow being last, exactly as the contract states it.
    func testActionRowOrder() {
        // `MAILCAL_OPEN_FIRST` opens the first row as soon as it loads, so the reading view is
        // reached without a tap on a list whose contents this test does not otherwise care about.
        let app = ShowcaseApp.launch(["MAILCAL_OPEN_FIRST": "1"])

        let reply = app.buttons["Reply"]
        XCTAssertTrue(
            reply.waitForExistence(timeout: ShowcaseApp.timeout),
            "no message was opened, so there is no action row to measure"
        )

        let expected = ["Reply", "Reply all", "Forward", "Archive", "Delete", "More actions"]
        let order = ShowcaseApp.leftToRight(expected, in: app)

        // Counted before anything is sliced out of it, and the test gives up here rather than
        // carrying on. A short array under a subscript is a **trap**, not a failed assertion: it
        // takes the whole test runner down, and the crash report names Swift's bounds check rather
        // than the row that was not drawn.
        guard order.count == expected.count else {
            return XCTFail("the action row drew \(order.count) of \(expected.count): \(order)")
        }
        XCTAssertEqual(
            Array(order.prefix(3)), ["Reply", "Reply all", "Forward"],
            "the row does not open with reply, reply-all and forward"
        )
        XCTAssertEqual(
            Set(order[3 ... 4]), ["Archive", "Delete"],
            "archive and delete are not the pair before the overflow"
        )
        XCTAssertEqual(
            order.last, "More actions",
            "the overflow button is not last of all"
        )
    }
}
