// Which categories the Settings hub draws (docs/settings.md).
//
// Writing style is the one category the core decides at runtime: it is drawn only while AI has
// somewhere to go (`WritingStyleSnapshot.route`), and the showcase has neither an own endpoint nor
// Allodia's relay. A category that stays when it should go opens a pane whose one button can only
// fail.

import XCTest

@MainActor
final class SettingsCategoryTests: XCTestCase {
    /// Absent, and nothing left in its place: Notifications follows Signatures directly.
    ///
    /// Notifications is waited for as well as Signatures because the hub is a lazy list: a row
    /// below the fold is not in the tree at all, so an absence is only evidence between two rows
    /// that are.
    func testWritingStyleIsAbsentWithoutAnEndpoint() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "settings"])

        let signatures = app.staticTexts["Signatures"]
        let notifications = app.staticTexts["Notifications"]
        XCTAssertTrue(signatures.waitForExistence(timeout: ShowcaseApp.timeout), "Settings did not open")
        XCTAssertTrue(notifications.waitForExistence(timeout: ShowcaseApp.timeout), "the hub drew no Notifications")
        XCTAssertFalse(app.staticTexts["Writing style"].exists, "Writing style was drawn with no AI route")
        XCTAssertLessThan(signatures.frame.minY, notifications.frame.minY, "the hub is out of order")
    }
}
