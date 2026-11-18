// The action row above an open message (`docs/reading-actions.md`): what its overflow reaches.
//
// The row's order, and the sun/moon button a dark app adds to it, are facts about one screen's
// layout, held without a launch by MailcalKit's `ReadingActionRowLayoutTests`. What is left here
// needs the running app: a sheet another process draws.

import XCTest

@MainActor
final class ReadingActionRowTests: XCTestCase {
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
