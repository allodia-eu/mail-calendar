// The action row above an open message (`docs/reading-actions.md`).
//
// The contract fixes an order, and an order is a fact about the screen: reply, reply-all and
// forward at the start, archive and delete at the end, the overflow button last of all. Nothing
// below the UI can observe it.

import XCTest

@MainActor
final class ReadingActionRowTests: XCTestCase {
    /// The row's order, measured left to right.
    ///
    /// Which of archive and delete comes first is each platform's own, so the assertion is on the
    /// three groups and on the overflow being last, exactly as the contract states it.
    func testActionRowOrder() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.openFirstMessage(app)
        try app.wait(.button("Reply"), "no message was opened, so there is no action row to measure")

        let expected = ["Reply", "Reply all", "Forward", "Archive", "Delete", "More actions"]
        let order = try ShowcaseApp.leftToRight(expected, in: app)

        // Counted before anything is sliced out of it: a short array under a subscript is a trap
        // that takes the whole test runner down, not a failed assertion.
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
        XCTAssertEqual(order.last, "More actions", "the overflow button is not last of all")
    }

    /// Print, behind the overflow, reaches the system's print sheet.
    ///
    /// The page is laid out in a web view nobody sees, so the one thing a reader can observe is the
    /// sheet arriving; a web view that never finished loading leaves the press doing nothing.
    func testPrintOpensTheSystemPrintSheet() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.openFirstMessage(app)

        try app.press(.button("More actions"), "no action row")
        // The body has to have arrived before there is anything to print.
        try app.wait(.button("Print…").enabled(true), "the menu offers no Print")
        // The sheet's own printer row, which no screen of ours draws.
        let printer = Filter.textLike("Printer")
        XCTAssertFalse(try app.exists(printer), "something on screen already reads as the print sheet")
        try app.press(.button("Print…"))

        try app.wait(printer, "no print sheet appeared")
    }
}
