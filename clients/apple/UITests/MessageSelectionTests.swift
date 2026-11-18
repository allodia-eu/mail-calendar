// Selecting messages on iPhone and iPad (`docs/list-selection.md`).
//
// The list's selection is the toolkit's, so what a tap does inside the mode is UIKit's and is not
// restated here. What is the app's: the mode is the list's edit mode, its bar comes and goes with
// it, Select all and ⌘A pick every listed row, ⌘A in a focused field stays the field's, and a row
// opened by a tap opens again by the next one.
//
// The bar's Archive shares its name with the Archive folder, so the bar is found by Delete, which
// no folder is called.

import UIKit
import XCTest

@MainActor
final class MessageSelectionTests: XCTestCase {
    private let rows = Filter().type("UICollectionViewCell").has(.role("AXStaticText"))
    private let trash = Filter.button("Delete")

    func testSelectModeBringsTheBarAndSelectAllPicksEveryRow() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)
        try app.wait(rows, "no rows")

        try app.press(.button("Select"))
        XCTAssertFalse(
            try app.wait(trash, timeout: 5, "the bar did not come with the mode").enabled,
            "the bar acts with nothing selected"
        )

        try app.press(.button("Select all"))
        XCTAssertTrue(try app.wait(trash).enabled, "Select all picked nothing")
        XCTAssertGreaterThan(selectedRows(in: app), 1, "the rows do not report themselves selected")

        try app.press(.button("Done"))
        try app.waitGone(trash, "the bar outlived the mode")
        XCTAssertEqual(selectedRows(in: app), 0, "leaving the mode kept the selection")
    }

    /// SwiftUI's list answers the system's Select All on the Mac only, so on a touch screen the
    /// chord is the bar's, which stands only while the mode is on.
    ///
    /// ⚠️ The first chord also tells the simulator a keyboard is attached, and is not delivered as
    /// one. The chord outside the mode is that first one, and asserts that it selects nothing.
    func testCommandAInSelectModePicksEveryRow() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)
        try app.wait(rows, "no rows")

        app.input.typeKey("a", modifierFlags: .command)
        try app.settle()
        XCTAssertFalse(try app.exists(.button("Done")), "⌘A outside the mode entered it")

        try app.press(.button("Select"))
        try app.wait(trash, timeout: 5, "the bar did not come with the mode")
        app.input.typeKey("a", modifierFlags: .command)

        XCTAssertTrue(
            try app.appears(trash.enabled(true), timeout: 5), "⌘A picked nothing"
        )
        XCTAssertGreaterThan(selectedRows(in: app), 1, "⌘A picked one row at most")
    }

    /// The reason the chord is bound where it is: a field the user is typing in keeps it. On the
    /// iPad the mode, and so the bar's ⌘A, is on as well; the iPhone never shows the two together,
    /// since the mode hides the search field and a search hides the toolbar's Select.
    func testCommandAInTheSearchFieldStaysTheFields() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)
        ShowcaseApp.attachHardwareKeyboard(app)
        if UIDevice.current.userInterfaceIdiom == .pad {
            try app.press(.button("Select"))
        }
        // Typed with the keyboard, because the chord under test is what the keyboard sends.
        let field = try app.wait(.text("Search mail").role("AXSearchField"))
        try app.tap(field)
        app.input.typeText("lunch")

        app.input.typeKey("a", modifierFlags: .command)
        app.input.typeText("x")
        try app.settle()

        XCTAssertEqual(try app.read(field).value, "x", "⌘A did not select the field's text")
        XCTAssertFalse(
            try app.query(trash).contains(where: \.enabled), "⌘A in the search field selected mail"
        )
    }

    /// A tap opens a row by selecting it, so Back has to let go of the row: otherwise the same
    /// tap changes nothing and the message it names never opens again.
    func testARowOpensAgainAfterBack() throws {
        try XCTSkipIf(
            UIDevice.current.userInterfaceIdiom == .pad, "the iPad reads beside the list, with no Back"
        )
        let app = try ShowcaseApp.launch()
        let reply = Filter.button("Reply")
        try ShowcaseApp.openMessage(containing: "Your June usage report", in: app)
        try app.wait(reply, "the row did not open")

        try ShowcaseApp.back(app)
        try app.waitGone(reply, "Back did not return to the list")

        try ShowcaseApp.openMessage(containing: "Your June usage report", in: app)
        try app.wait(reply, "the row did not open again")
    }

    /// How many rows report themselves selected. Asked of XCUITest: a cell is a container rather
    /// than an element of its own, and the accessibility runtime is what gives a container's
    /// selection to assistive technology.
    private func selectedRows(in app: App) -> Int {
        app.input.cells.matching(NSPredicate(format: "isSelected == true")).count
    }
}
