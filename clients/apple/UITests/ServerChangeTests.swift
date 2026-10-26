// The question a calendar paused for a server on another site asks (docs/account-autodetect.md
// rule 14): "Review" above the calendar opens it, "Not now" leaves the calendar paused, and only
// "Allow new server" asks the core to move anything.
//
// The pause is staged by MAILCAL_SHOWCASE_SERVER_CHANGE, which the core reads, so everything from
// the connectivity snapshot to the sheet is real. A showcase account has no servers to change, so
// the core refuses the allow, and that refusal is what proves the button reached it.

import XCTest

@MainActor
final class ServerChangeTests: XCTestCase {
    func testReviewAsksAndNotNowLeavesTheCalendarPaused() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_SERVER_CHANGE": "1"])
        ShowcaseApp.tap(app.buttons["Calendar"])
        let review = app.buttons["Review"]
        let title = app.staticTexts["Your calendar server has changed"]

        ShowcaseApp.tap(review)
        XCTAssertTrue(title.waitForExistence(timeout: ShowcaseApp.timeout), "Review asked nothing")
        XCTAssertTrue(app.staticTexts["Before: example.com"].exists, "the old site is not named")
        XCTAssertTrue(app.staticTexts["Now: example.net"].exists, "the new site is not named")

        ShowcaseApp.tap(app.buttons["Not now"])
        XCTAssertTrue(title.waitForNonExistence(timeout: ShowcaseApp.timeout), "Not now kept it open")
        XCTAssertTrue(review.exists, "Not now lifted the pause")
    }

    func testARefusedAllowSaysSoAndKeepsTheQuestionOpen() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_SERVER_CHANGE": "1"])
        ShowcaseApp.tap(app.buttons["Calendar"])
        ShowcaseApp.tap(app.buttons["Review"])
        let title = app.staticTexts["Your calendar server has changed"]
        XCTAssertTrue(title.waitForExistence(timeout: ShowcaseApp.timeout), "Review asked nothing")

        ShowcaseApp.tap(app.buttons["Allow new server"])
        let refused = app.staticTexts[
            "The new server couldn’t be reached, so nothing was changed. Please try again later."
        ]
        XCTAssertTrue(
            refused.waitForExistence(timeout: ShowcaseApp.timeout), "the refusal was not said"
        )
        XCTAssertTrue(title.exists, "a refused allow closed the question")
    }
}
