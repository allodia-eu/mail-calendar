// The invitation card's answer buttons take a press across their whole face.
//
// The meeting-day preview under them lays out the whole day and slides it up behind a clip, and a
// SwiftUI clip crops only the drawing. Whether anything of the hidden hours still sits over the
// buttons is a question about live hit testing, which only a running screen answers: the
// accessibility tree reports the button under the point either way.
//
// The showcase invitation is the fixture because its day has events before the preview's span
// (09:30 and 11:00 against a 14:30 meeting), which is the shape that puts a block above the grid.

import XCTest

@MainActor
final class InvitationAnswerTests: XCTestCase {
    func testTheAnswerButtonsTakeAPressAwayFromTheirLabel() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.openMessage(containing: "kickoff", in: app)

        // Each answer differs from the one before, so each press has to move the line. Decline is
        // left out: a declined meeting leaves the calendar, and the preview under test with it.
        let presses = [
            ("Answer maybe to this invitation", "You answered tentatively"),
            ("Accept this invitation", "You accepted"),
        ]
        for (name, line) in presses {
            let button = app.buttons[name]
            XCTAssertTrue(button.waitForExistence(timeout: ShowcaseApp.timeout), "no '\(name)'")
            let hittable = XCTNSPredicateExpectation(
                predicate: NSPredicate(format: "isHittable == true"), object: button
            )
            XCTAssertEqual(XCTWaiter().wait(for: [hittable], timeout: ShowcaseApp.timeout), .completed)
            // Near the trailing edge, off the word: the part of the button a block above the
            // preview covers and the label does not.
            button.coordinate(withNormalizedOffset: CGVector(dx: 0.92, dy: 0.5)).tap()
            XCTAssertTrue(
                app.staticTexts[line].waitForExistence(timeout: ShowcaseApp.timeout),
                "'\(name)' did not take a press near its trailing edge"
            )
        }
    }
}
