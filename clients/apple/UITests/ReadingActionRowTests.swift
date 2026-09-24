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
        let app = ShowcaseApp.launch()
        ShowcaseApp.openFirstMessage(app)

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

    /// Print, behind the overflow, reaches the system's print sheet.
    ///
    /// The page is laid out in a web view nobody sees, so the one thing a reader can observe is the
    /// sheet arriving; a web view that never finished loading leaves the tap doing nothing at all.
    func testPrintOpensTheSystemPrintSheet() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.openFirstMessage(app)

        let more = app.buttons["More actions"]
        XCTAssertTrue(more.waitForExistence(timeout: ShowcaseApp.timeout), "no action row")
        more.tap()
        let print = app.buttons["Print…"]
        XCTAssertTrue(print.waitForExistence(timeout: ShowcaseApp.timeout), "the menu offers no Print")
        // The body has to have arrived before there is anything to print.
        let enabled = NSPredicate(format: "isEnabled == true")
        wait(for: [expectation(for: enabled, evaluatedWith: print)], timeout: ShowcaseApp.timeout)
        // The sheet's own printer row, which no screen of ours draws.
        let printer = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label BEGINSWITH 'Printer'")).firstMatch
        XCTAssertFalse(printer.exists, "something on screen already reads as the print sheet")
        print.tap()

        XCTAssertTrue(printer.waitForExistence(timeout: ShowcaseApp.timeout), "no print sheet appeared")
    }
}
