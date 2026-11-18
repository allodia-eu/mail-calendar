// The sun/moon button on the reading view's action row (`docs/reading-dark-mode.md`).
//
// Two facts about the screen that nothing below the UI can see: the button exists only while the
// app is dark, and a seventh button on a phone's row still leaves the row inside the screen. A
// row wider than the screen pushes the whole reading view past both edges of an iPhone.
//
// The appearance is set with the `MAILCAL_APPEARANCE` launch hook rather than the simulator's own,
// so the result does not depend on how the simulator was left.

import XCTest

@MainActor
final class ReadingDarkModeTests: XCTestCase {
    private let toLight = "Show messages with a light background"
    private let toDark = "Show messages with a dark background"

    /// A light app has nothing to switch, so the row draws no button for it.
    func testALightAppDrawsNoSunOrMoon() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_APPEARANCE": "light"])
        try ShowcaseApp.openFirstMessage(app)
        try app.wait(
            .button("Reply"), "no message was opened, so there is no action row to look at"
        )
        XCTAssertFalse(try app.exists(.button(toLight)), "a light app offers a light background")
        XCTAssertFalse(try app.exists(.button(toDark)), "a light app offers a dark background")
    }

    /// In a dark app the button opens the trailing group, the row still fits the screen, and a
    /// press turns the sun into the moon.
    func testADarkAppDrawsTheButtonBeforeArchiveAndKeepsTheRowOnScreen() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_APPEARANCE": "dark"])
        try ShowcaseApp.openFirstMessage(app)
        let sun = try app.wait(.button(toLight), "a dark app draws no sun on the action row")

        let row = ["Reply", "Reply all", "Forward", toLight, "Archive", "Delete", "More actions"]
        let order = try ShowcaseApp.leftToRight(row, in: app)
        guard order.count == row.count else {
            return XCTFail("the action row drew \(order.count) of \(row.count) on screen: \(order)")
        }
        XCTAssertEqual(order[3], toLight, "the button does not open the trailing group: \(order)")

        // Every button wholly inside the window: a row wider than the screen is not clipped, it
        // widens the column it sits in and takes the header and the message with it.
        // The row is the buttons on the sun's own line; a label such as Delete is also a swipe
        // action in the list behind it.
        let screen = app.input.windows.firstMatch.frame
        let line = try XCTUnwrap(sun.frame, "the sun has no frame").midY
        let buttons = try row.flatMap { try app.query(.button($0)) }
            .compactMap { button in button.frame.map { (button.label, $0) } }
            .filter { abs($0.1.midY - line) < 4 }
        XCTAssertEqual(buttons.count, row.count, "the action row is not one line of \(row.count)")
        for (label, frame) in buttons {
            XCTAssertTrue(screen.contains(frame), "\(label) runs off the screen: \(frame) in \(screen)")
            XCTAssertGreaterThanOrEqual(frame.width, 44, "\(label) is narrower than a finger")
        }

        try app.activate(sun)
        try app.wait(
            .button(toDark), "pressing the sun did not send the message back to its light page"
        )
        // The preference persists, so put it back for whichever test runs next.
        try app.press(.button(toDark))
        try app.wait(.button(toLight))
    }
}
