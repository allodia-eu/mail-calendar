// A banner sits under the navigation bar it shares the screen with, never over it.
//
// On iPhone an inset outside a tab's navigation stack does not move that stack's bar down, so a
// banner on the app's root drew over the bar and its buttons (`Mailcal.Banners.swift`). The offline
// banner stands for all of them, being the one a launch hook can raise.
//
// ⚠️ Measured by frames, not by `isHittable`: with the banner drawn over the bar, XCUITest still
// answered that the buttons under it were hittable.

import XCTest

@MainActor
final class BannerPlacementTests: XCTestCase {
    func testTheBannerSitsBelowTheBarsButtons() {
        let app = ShowcaseApp.launch(["MAILCAL_OFFLINE": "1"])
        ShowcaseApp.showMailbox(app)
        let banner = app.staticTexts["You're offline, showing saved mail."]
        XCTAssertTrue(
            banner.waitForExistence(timeout: ShowcaseApp.timeout),
            "the offline banner did not come up"
        )
        assertBelow(banner, ["Folders", "New Mail"], in: app)

        ShowcaseApp.tap(app.buttons["Calendar"])
        XCTAssertTrue(
            app.buttons["New Event"].waitForExistence(timeout: ShowcaseApp.timeout),
            "the calendar did not come up"
        )
        assertBelow(banner, ["New Event", "More actions"], in: app)
    }

    private func assertBelow(
        _ banner: XCUIElement, _ buttons: [String], in app: XCUIApplication,
        file: StaticString = #filePath, line: UInt = #line
    ) {
        for name in buttons {
            let button = app.buttons[name]
            XCTAssertTrue(button.exists, "no \(name) on the bar", file: file, line: line)
            XCTAssertGreaterThanOrEqual(
                banner.frame.minY, button.frame.maxY,
                "the banner is drawn over \(name)", file: file, line: line
            )
        }
    }
}
