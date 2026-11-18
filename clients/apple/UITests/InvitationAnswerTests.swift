// The invitation card's answer buttons take a press across their whole face.
//
// The meeting-day preview under them lays out the whole day and slides it up behind a clip, and a
// SwiftUI clip crops only the drawing. Whether anything of the hidden hours still sits over the
// buttons is a question about live hit testing, which only a real touch answers: the
// accessibility tree reports the button under the point either way, and the agent's press never
// goes through hit testing at all.
//
// The showcase invitation is the fixture because its day has events before the preview's span
// (09:30 and 11:00 against a 14:30 meeting), which is the shape that puts a block above the grid.

import XCTest

@MainActor
final class InvitationAnswerTests: XCTestCase {
    func testTheAnswerButtonsTakeAPressAwayFromTheirLabel() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.openMessage(containing: "kickoff", in: app)

        // Each answer differs from the one before, so each press has to move the line. Decline is
        // left out: a declined meeting leaves the calendar, and the preview under test with it.
        let presses = [
            ("Answer maybe to this invitation", "You answered tentatively"),
            ("Accept this invitation", "You accepted"),
        ]
        for (name, line) in presses {
            let button = try app.wait(.button(name), "no '\(name)'")
            // Near the trailing edge, off the word: the part of the button a block above the
            // preview covers and the label does not.
            try app.tap(button, at: CGVector(dx: 0.92, dy: 0.5))
            try app.wait(.text(line), "'\(name)' did not take a press near its trailing edge")
        }
    }
}
