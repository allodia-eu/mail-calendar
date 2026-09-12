// The calendar's pinch (`docs/calendar.md`).
//
// This is the one thing no other mechanism in this repository can do. `idb` has no pinch, the
// Windows UI suite drives UI Automation rather than touches, and the Rust and Swift unit tests see
// the zoom arithmetic but never the gesture that feeds it. A pinch that never reaches the grid is
// therefore invisible to every gate except this one, and that is exactly what had happened: the
// recognizer sat on a SwiftUI overlay, which is a sibling of the content rather than its ancestor,
// so UIKit never offered it a touch and the iOS grid could not be zoomed at all.
//
// ⚠️ `XCUIElement.pinch(withScale:velocity:)` moves the fingers a good deal less than the scale
// suggests: at `withScale: 3` they end about 20 x 37 points apart, under the 48-point floor each
// axis needs before its scale means anything (`CalendarZoomGesture.minSpread`), so nothing moves
// and the test reads as a broken app. The scales below are measured, not guessed.

import XCTest

@MainActor
final class CalendarPinchTests: XCTestCase {
    /// Spreading the fingers gives more room per hour; pinching them together gives less.
    ///
    /// Measured off the hour ruler's own labels, so the assertion is on what the grid drew rather
    /// than on a number the view model agreed with itself about. The ruler is the pinned column on
    /// the left, which is what separates the hour "09" from the ninth of the month.
    func testPinchZoomsTheHourAxis() {
        let app = ShowcaseApp.launch(["MAILCAL_CALENDAR": "1"])
        XCTAssertTrue(
            app.staticTexts.matching(NSPredicate(format: "label == %@", "09")).firstMatch
                .waitForExistence(timeout: ShowcaseApp.timeout),
            "the calendar grid did not come up"
        )

        let grid = app.windows.firstMatch
        let resting = hourPitch(app)
        XCTAssertGreaterThan(resting, 0, "the hour ruler drew no hours to measure")

        grid.pinch(withScale: 10, velocity: 3)
        let spread = hourPitch(app)
        XCTAssertGreaterThan(
            spread, resting * 1.2,
            "spreading the fingers did not give the hours more room (\(resting) -> \(spread))"
        )

        grid.pinch(withScale: 0.2, velocity: -3)
        let squeezed = hourPitch(app)
        XCTAssertLessThan(
            squeezed, spread * 0.9,
            "pinching the fingers together did not take room back (\(spread) -> \(squeezed))"
        )
    }

    /// The distance between two neighbouring hour labels in the pinned ruler.
    ///
    /// `08` and `09` by name, not the ruler's nth and (n+1)th: enumerating every label on the grid
    /// walks hundreds of elements whose snapshots go stale mid-walk, and the two hours nearest the
    /// middle of the viewport are the ones a zoom anchored on the screen's centre keeps on screen.
    /// They are zero-padded, which is what tells them apart from the day-of-month numbers in the
    /// header; the ruler runs `00`–`23` under the app's own clock setting.
    private func hourPitch(_ app: XCUIApplication) -> CGFloat {
        let eight = app.staticTexts.matching(NSPredicate(format: "label == %@", "08")).firstMatch
        let nine = app.staticTexts.matching(NSPredicate(format: "label == %@", "09")).firstMatch
        guard eight.exists, nine.exists else { return 0 }
        return nine.frame.midY - eight.frame.midY
    }
}
