// The manual form's account-type menu: beside its label while both fit, beneath it once the longest
// kind is chosen on a phone. Squeezed beside the menu, the label wrapped into a column of single
// letters and left a tall empty gap above the form.

import XCTest

@MainActor
final class AccountSetupKindPickerTests: XCTestCase {
    func testTheLongestKindKeepsItsLabelOnOneLine() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "setup-manual"])
        // The menu's label reads its current kind after "Account type".
        let menus = try app.waitAll(.textLike("Account type").role("AXButton"), "no account type menu")
        let menu = try XCTUnwrap(
            menus.first { $0.label.hasPrefix("Account type") }, "no account type menu"
        )
        // The form is a sheet, and a menu opened while the sheet is still coming up closes again.
        try app.settle()
        try app.activate(menu)
        try app.press(
            .button("Calendar and contacts (CalDAV/CardDAV)"), "the menu did not open"
        )

        let label = try app.wait(.text("Account type").role("AXStaticText"), "the label went missing")
        let notes = try app.query(.textLike("For a server that holds your calendar"))
        let note = try XCTUnwrap(
            notes.first { $0.label.hasPrefix("For a server that holds your calendar") },
            "the calendar-and-contacts form did not open"
        )
        let labelFrame = try XCTUnwrap(label.frame, "the label has no frame")
        let noteFrame = try XCTUnwrap(note.frame, "the note has no frame")
        // One line of text, not a column of letters.
        XCTAssertGreaterThan(labelFrame.width, labelFrame.height, "the label wrapped")
        // The menu follows the label and the form follows the menu, with no gap between them.
        XCTAssertLessThan(noteFrame.minY - labelFrame.maxY, 120, "a gap opened above the form")
    }
}
