// The sun/moon button on the reading view's action row (`docs/reading-dark-mode.md`).
//
// Two facts about the screen that nothing below the UI can see: the button exists only while the
// app is dark, and a seventh button on a phone's row still leaves the row inside the screen. The
// second is not hypothetical: the first version of the button pushed the row, and with it the
// whole reading view, past both edges of an iPhone.
//
// The appearance is set with the `MAILCAL_APPEARANCE` launch hook rather than the simulator's own,
// so the result does not depend on how the simulator was left.

import XCTest

@MainActor
final class ReadingDarkModeTests: XCTestCase {
    private let toLight = "Show messages with a light background"
    private let toDark = "Show messages with a dark background"

    /// A light app has nothing to switch, so the row draws no button for it.
    func testALightAppDrawsNoSunOrMoon() {
        let app = ShowcaseApp.launch(["MAILCAL_APPEARANCE": "light"])
        ShowcaseApp.openFirstMessage(app)
        XCTAssertTrue(
            app.buttons["Reply"].waitForExistence(timeout: ShowcaseApp.timeout),
            "no message was opened, so there is no action row to look at"
        )
        XCTAssertFalse(app.buttons[toLight].exists, "a light app offers a light background")
        XCTAssertFalse(app.buttons[toDark].exists, "a light app offers a dark background")
    }

    /// In a dark app the button opens the trailing group, the row still fits the screen, and a
    /// press turns the sun into the moon.
    func testADarkAppDrawsTheButtonBeforeArchiveAndKeepsTheRowOnScreen() {
        let app = ShowcaseApp.launch(["MAILCAL_APPEARANCE": "dark"])
        ShowcaseApp.openFirstMessage(app)
        let sun = app.buttons[toLight]
        XCTAssertTrue(
            sun.waitForExistence(timeout: ShowcaseApp.timeout),
            "a dark app draws no sun on the action row"
        )

        let row = ["Reply", "Reply all", "Forward", toLight, "Archive", "Delete", "More actions"]
        let order = ShowcaseApp.leftToRight(row, in: app)
        guard order.count == row.count else {
            return XCTFail("the action row drew \(order.count) of \(row.count) on screen: \(order)")
        }
        XCTAssertEqual(order[3], toLight, "the button does not open the trailing group: \(order)")

        // Every button wholly inside the window: a row wider than the screen is not clipped, it
        // widens the column it sits in and takes the header and the message with it.
        // The row is the buttons on the sun's own line; a label such as Delete is also a swipe
        // action in the list behind it.
        let screen = app.windows.firstMatch.frame
        let line = sun.frame.midY
        let buttons = app.buttons
            .matching(NSPredicate(format: "label IN %@", row))
            .allElementsBoundByIndex
            .filter { abs($0.frame.midY - line) < 4 }
        XCTAssertEqual(buttons.count, row.count, "the action row is not one line of \(row.count)")
        for button in buttons {
            let frame = button.frame
            XCTAssertTrue(screen.contains(frame), "\(button.label) runs off the screen: \(frame) in \(screen)")
            XCTAssertGreaterThanOrEqual(frame.width, 44, "\(button.label) is narrower than a finger")
        }

        sun.tap()
        XCTAssertTrue(
            app.buttons[toDark].waitForExistence(timeout: ShowcaseApp.timeout),
            "pressing the sun did not send the message back to its light page"
        )
        // The preference persists, so put it back for whichever test runs next.
        app.buttons[toDark].tap()
        XCTAssertTrue(sun.waitForExistence(timeout: ShowcaseApp.timeout))
    }
}
