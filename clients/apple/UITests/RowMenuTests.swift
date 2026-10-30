// A conversation row's menu and how a conversation expands (`docs/row-actions.md`), on the
// showcase's one conversation: Tom's request, the owner's reply filed in Sent, and Tom's unread
// follow-up, which is the newest message.
//
// A unit test sees what the core answers and what the client would draw from it; only this sees
// what a long press actually raises, and which rows the list holds once the user moves on.

import XCTest

@MainActor
final class RowMenuTests: XCTestCase {
    /// The conversation's subject, shown on its row only.
    private let conversation = "Q3 launch"
    /// The owner's reply's preview, shown only once the conversation is expanded.
    private let reply = "Checklist looks solid"

    /// A conversation offers what one of its messages does: the newest is unread and received,
    /// so it can be answered, marked read, flagged and reported, and the conversation archived.
    func testAConversationOffersWhatAMessageDoes() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)
        let row = row(mentioning: conversation, in: app)
        row.press(forDuration: 1.2)

        for label in ["Reply", "Reply all", "Forward", "Mark as read", "Flag", "Mark as Spam",
                      "Archive", "Move to Trash"] {
            XCTAssertTrue(
                app.buttons[label].waitForExistence(timeout: 5),
                "the conversation's menu has no '\(label)'"
            )
        }
        XCTAssertTrue(app.buttons["Reply"].isEnabled, "the newest message was received")
        XCTAssertFalse(
            app.buttons["Archive conversation"].exists,
            "the conversation's archive is the message menu's Archive"
        )
    }

    /// Opening the conversation lays its messages out in the list; opening any other row folds
    /// them away again, so a conversation the user has left does not stay open behind them.
    func testLeavingAConversationFoldsItAway() throws {
        try XCTSkipIf(
            UIDevice.current.userInterfaceIdiom == .pad, "the iPad reads beside the list, with no Back"
        )
        let app = ShowcaseApp.launch()
        ShowcaseApp.openMessage(containing: conversation, in: app)
        back(in: app)
        XCTAssertTrue(
            expanded(app).waitForExistence(timeout: ShowcaseApp.timeout),
            "opening the conversation did not lay out its messages"
        )

        ShowcaseApp.openMessage(containing: "Your June usage report", in: app)
        back(in: app)
        XCTAssertTrue(gone(expanded(app)), "the conversation stayed open after the user left it")
    }

    private func row(mentioning text: String, in app: XCUIApplication) -> XCUIElement {
        let mentions = NSPredicate(format: "label CONTAINS[c] %@", text)
        let row = app.cells.containing(mentions).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: ShowcaseApp.timeout), "no row for '\(text)'")
        return row
    }

    /// The owner's reply as a row of its own, which exists only while the conversation is open.
    private func expanded(_ app: XCUIApplication) -> XCUIElement {
        app.cells.containing(NSPredicate(format: "label CONTAINS[c] %@", reply)).firstMatch
    }

    private func back(in app: XCUIApplication) {
        let back = app.navigationBars.buttons.element(boundBy: 0)
        XCTAssertTrue(back.waitForExistence(timeout: ShowcaseApp.timeout), "no way back to the list")
        back.tap()
    }

    private func gone(_ element: XCUIElement) -> Bool {
        let gone = NSPredicate(format: "exists == false")
        let expectation = XCTNSPredicateExpectation(predicate: gone, object: element)
        return XCTWaiter().wait(for: [expectation], timeout: 5) == .completed
    }
}
