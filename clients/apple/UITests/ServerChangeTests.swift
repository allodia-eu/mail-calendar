// The question a calendar paused for a server on another site asks (docs/account-autodetect.md
// rule 14): the notice's "Review" opens it, "Not now" leaves the calendar paused, and only
// "Allow new server" asks the core to move anything.
//
// The pause is staged by MAILCAL_SHOWCASE_SERVER_CHANGE, which the core reads, so everything from
// the notice to the sheet is real. A showcase account has no servers to change, so the core
// refuses the allow, and that refusal is what proves the button reached it.

import UIKit
import XCTest

@MainActor
final class ServerChangeTests: XCTestCase {
    private let title = Filter.text("Your calendar server has changed")

    func testReviewAsksAndNotNowLeavesTheCalendarPaused() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SERVER_CHANGE": "1"])
        try showCalendarNotice(app)

        try app.press(.button("Review"))
        try app.wait(title, "Review asked nothing")
        XCTAssertTrue(try app.exists(.text("Before: example.com")), "the old site is not named")
        XCTAssertTrue(try app.exists(.text("Now: example.net")), "the new site is not named")

        try app.press(.button("Not now"))
        try app.waitGone(title, timeout: ShowcaseApp.timeout, "Not now kept it open")
        XCTAssertTrue(try app.exists(.button("Review")), "Not now lifted the pause")
    }

    func testARefusedAllowSaysSoAndKeepsTheQuestionOpen() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SERVER_CHANGE": "1"])
        try showCalendarNotice(app)
        try app.press(.button("Review"))
        try app.wait(title, "Review asked nothing")

        try app.press(.button("Allow new server"))
        let refused = try app.wait(
            .text("The new server couldn’t be reached, so nothing was changed. Please try again later."),
            "the refusal was not said"
        )
        // Said is not seen: on a phone the sheet opens at half height, where a line at the foot of
        // the scrolled text sits under the buttons. So the frames are compared.
        let notNow = try app.wait(.button("Not now"))
        XCTAssertLessThanOrEqual(
            refused.frame?.maxY ?? .infinity, notNow.frame?.minY ?? 0,
            "the refusal is hidden behind the buttons"
        )
        XCTAssertTrue(try app.exists(title), "a refused allow closed the question")
    }

    /// Brings the paused calendar's notice, and with it "Review", on screen (docs/notices.md).
    ///
    /// The iPad draws the notice as a card at the top of the sidebar, its button on it. The iPhone
    /// draws one row under the bar, and the button is in the sheet that row opens.
    private func showCalendarNotice(_ app: App) throws {
        try app.press(.button("Calendar"))
        if UIDevice.current.userInterfaceIdiom != .pad {
            try app.press(.id("notice"))
        }
    }
}
