// The manual form's account-type menu: beside its label while both fit, beneath it once the longest
// kind is chosen on a phone. Squeezed beside the menu, the label wrapped into a column of single
// letters and left a tall empty gap above the form.

import XCTest

@MainActor
final class AccountSetupKindPickerTests: XCTestCase {
    func testTheLongestKindKeepsItsLabelOnOneLine() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "setup-manual"])
        let menu = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Account type'")).firstMatch
        XCTAssertTrue(menu.waitForExistence(timeout: ShowcaseApp.timeout), "no account type menu")
        ShowcaseApp.tap(menu)
        let dav = app.buttons["Calendar and contacts (CalDAV/CardDAV)"]
        XCTAssertTrue(dav.waitForExistence(timeout: ShowcaseApp.timeout), "the menu did not open")
        ShowcaseApp.tap(dav)

        let label = app.staticTexts["Account type"]
        XCTAssertTrue(label.waitForExistence(timeout: ShowcaseApp.timeout), "the label went missing")
        let note = app.staticTexts.matching(
            NSPredicate(format: "label BEGINSWITH 'For a server that holds your calendar'")
        ).firstMatch
        XCTAssertTrue(note.exists, "the calendar-and-contacts form did not open")
        // One line of text, not a column of letters.
        XCTAssertGreaterThan(label.frame.width, label.frame.height, "the label wrapped")
        // The menu follows the label and the form follows the menu, with no gap between them.
        XCTAssertLessThan(note.frame.minY - label.frame.maxY, 120, "a gap opened above the form")
    }
}
