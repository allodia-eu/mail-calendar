// Selecting messages on iPhone and iPad (`docs/list-selection.md`).
//
// The list's selection is the toolkit's, so what a tap does inside the mode is UIKit's and is not
// restated here. What is the app's: the mode is the list's edit mode, its bar comes and goes with
// it, Select all and ⌘A pick every listed row, ⌘A in a focused field stays the field's, and a row
// opened by a tap opens again by the next one.
//
// The bar's Archive shares its name with the Archive folder, so the bar is found by Move to Trash,
// which no folder is called.

import UIKit
import XCTest

@MainActor
final class MessageSelectionTests: XCTestCase {
    func testSelectModeBringsTheBarAndSelectAllPicksEveryRow() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)
        XCTAssertTrue(app.cells.firstMatch.waitForExistence(timeout: ShowcaseApp.timeout), "no rows")

        ShowcaseApp.tap(app.buttons["Select"])
        let trash = app.buttons["Move to Trash"]
        XCTAssertTrue(trash.waitForExistence(timeout: 5), "the bar did not come with the mode")
        XCTAssertFalse(trash.isEnabled, "the bar acts with nothing selected")

        ShowcaseApp.tap(app.buttons["Select all"])
        XCTAssertTrue(trash.isEnabled, "Select all picked nothing")
        XCTAssertGreaterThan(selectedRows(in: app), 1, "the rows do not report themselves selected")

        ShowcaseApp.tap(app.buttons["Done"])
        XCTAssertTrue(gone(trash), "the bar outlived the mode")
        XCTAssertEqual(selectedRows(in: app), 0, "leaving the mode kept the selection")
    }

    /// SwiftUI's list answers the system's Select All on the Mac only, so on a touch screen the
    /// chord is the bar's, which stands only while the mode is on.
    ///
    /// ⚠️ The first chord also tells the simulator a keyboard is attached, and is not delivered as
    /// one. The chord outside the mode is that first one, and asserts that it selects nothing.
    func testCommandAInSelectModePicksEveryRow() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)
        XCTAssertTrue(app.cells.firstMatch.waitForExistence(timeout: ShowcaseApp.timeout), "no rows")

        app.typeKey("a", modifierFlags: .command)
        XCTAssertFalse(app.buttons["Done"].exists, "⌘A outside the mode entered it")

        ShowcaseApp.tap(app.buttons["Select"])
        app.typeKey("a", modifierFlags: .command)

        XCTAssertTrue(app.buttons["Move to Trash"].isEnabled, "⌘A picked nothing")
        XCTAssertGreaterThan(selectedRows(in: app), 1, "⌘A picked one row at most")
    }

    /// The reason the chord is bound where it is: a field the user is typing in keeps it. On the
    /// iPad the mode, and so the bar's ⌘A, is on as well; the iPhone never shows the two together,
    /// since the mode hides the search field and a search hides the toolbar's Select.
    func testCommandAInTheSearchFieldStaysTheFields() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)
        if UIDevice.current.userInterfaceIdiom == .pad {
            ShowcaseApp.tap(app.buttons["Select"])
        }
        let field = app.searchFields["Search mail"]
        ShowcaseApp.tap(field)
        field.typeText("lunch")

        app.typeKey("a", modifierFlags: .command)
        field.typeText("x")

        XCTAssertEqual(field.value as? String, "x", "⌘A did not select the field's text")
        let trash = app.buttons["Move to Trash"]
        XCTAssertFalse(trash.exists && trash.isEnabled, "⌘A in the search field selected mail")
    }

    /// A tap opens a row by selecting it, so Back has to let go of the row: otherwise the same
    /// tap changes nothing and the message it names never opens again.
    func testARowOpensAgainAfterBack() throws {
        try XCTSkipIf(
            UIDevice.current.userInterfaceIdiom == .pad, "the iPad reads beside the list, with no Back"
        )
        let app = ShowcaseApp.launch()
        ShowcaseApp.openMessage(containing: "Your June usage report", in: app)
        let reply = app.buttons["Reply"]
        XCTAssertTrue(reply.waitForExistence(timeout: ShowcaseApp.timeout), "the row did not open")

        app.navigationBars.buttons.element(boundBy: 0).tap()
        XCTAssertTrue(gone(reply), "Back did not return to the list")

        ShowcaseApp.openMessage(containing: "Your June usage report", in: app)
        XCTAssertTrue(reply.waitForExistence(timeout: ShowcaseApp.timeout), "the row did not open again")
    }

    private func selectedRows(in app: XCUIApplication) -> Int {
        app.cells.matching(NSPredicate(format: "isSelected == true")).count
    }

    private func gone(_ element: XCUIElement) -> Bool {
        let gone = NSPredicate(format: "exists == false OR isHittable == false")
        let expectation = XCTNSPredicateExpectation(predicate: gone, object: element)
        return XCTWaiter().wait(for: [expectation], timeout: 5) == .completed
    }
}
