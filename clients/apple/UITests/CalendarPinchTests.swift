// The calendar's pinch (`docs/calendar.md`).
//
// A pinch is a gesture, which only real input can give: `idb` has no pinch, the agent acts as
// VoiceOver does and has none either, and the Rust and Swift unit tests see the zoom arithmetic but
// never the gesture that feeds it. A pinch that never reaches the grid is therefore invisible to
// every gate except this one: the recognizer once sat on a SwiftUI overlay, a sibling of the
// content rather than its ancestor, so UIKit never offered it a touch.
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
    func testPinchZoomsTheHourAxis() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_CALENDAR": "1"])
        try app.wait(hour("09"), "the calendar grid did not come up")

        let grid = app.input.windows.firstMatch
        let resting = try hourPitch(app)
        XCTAssertGreaterThan(resting, 0, "the hour ruler drew no hours to measure")

        grid.pinch(withScale: 10, velocity: 3)
        try app.settle()
        let spread = try hourPitch(app)
        XCTAssertGreaterThan(
            spread, resting * 1.2,
            "spreading the fingers did not give the hours more room (\(resting) -> \(spread))"
        )

        grid.pinch(withScale: 0.2, velocity: -3)
        try app.settle()
        let squeezed = try hourPitch(app)
        XCTAssertLessThan(
            squeezed, spread * 0.9,
            "pinching the fingers together did not take room back (\(spread) -> \(squeezed))"
        )
    }

    /// An hour of the ruler. They are zero-padded, which is what tells them apart from the
    /// day-of-month numbers in the header; the ruler runs `00`–`23` under the app's own clock
    /// setting.
    ///
    /// Off screen included: a spread can carry one of the two past the edge, and its frame still
    /// says where it went.
    private func hour(_ label: String) -> Filter {
        .text(label).role("AXStaticText").hidden()
    }

    /// The distance between two neighbouring hour labels in the pinned ruler: `08` and `09`, the
    /// two hours nearest the middle of the viewport, which a zoom anchored on the screen's centre
    /// keeps on screen.
    private func hourPitch(_ app: App) throws -> CGFloat {
        guard let eight = try app.query(hour("08")).first?.frame,
              let nine = try app.query(hour("09")).first?.frame
        else { return 0 }
        return nine.midY - eight.midY
    }
}
