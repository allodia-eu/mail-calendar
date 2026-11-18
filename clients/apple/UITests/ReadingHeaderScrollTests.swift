// The reading header rides the message's scroll, and the action row does not
// (`docs/reading-actions.md`).
//
// Nothing below the UI can see this. The header travels inside the scroll view the body brings,
// on the `contentInset` the reading pane gives it, so whether it actually moves is a question
// about two live views agreeing, which only a running screen answers. The drag is a real finger;
// the frames it moves are read through the agent.

import XCTest

@MainActor
final class ReadingHeaderScrollTests: XCTestCase {
    /// ⚠️ **The message is the fixture, and not any message.** The showcase bodies are a few
    /// paragraphs each and fit a large phone's pane, so most of them give a swipe nothing to
    /// scroll and would pass these assertions over a header that never had anywhere to go. The
    /// invitation carries the RSVP card, which is part of the header and taller than the rest of
    /// it put together, so this one always outgrows the screen.
    ///
    /// Rotating instead would not do: landscape on a large iPhone is a **regular** width, which is
    /// the iPad's three-pane layout rather than the screen under test.
    func testTheHeaderScrollsAwayAndTheActionRowStays() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.openMessage(containing: "kickoff", in: app)

        let reply = Filter.button("Reply")
        try app.wait(reply, "no message was opened, so there is no reading view to scroll")
        // The recipients line, which every message in the dataset has and no other surface draws
        // while a message is open.
        let recipients = Filter.text("To:").role("AXStaticText")
        try app.wait(recipients, "the reading header drew no recipients line to follow")

        // ⚠️ **The header is on screen a good while before the scroll it rides exists.** It is a
        // subview of the body's scroll view, and that view has nothing to scroll until WebKit has
        // committed the document: its content is shorter than the pane until then, so a drag
        // rubber-bands back and leaves the header where it was. The body's own text enters the
        // accessibility tree once the document is laid out, so that is waited for first, on a
        // longer clock than the queries'. It is web content, which only XCUITest reads. The text
        // can still arrive a moment before the content size does, so the drag below is repeated
        // until the message answers it.
        let body = app.input.webViews.staticTexts
            .matching(NSPredicate(format: "label CONTAINS[c] %@", "in everyone"))
            .firstMatch
        guard body.waitForExistence(timeout: 3 * ShowcaseApp.timeout) else {
            return XCTFail("the message body never loaded, so there was nothing to scroll\n\(app.dump())")
        }

        let rowBefore = try app.wait(reply).frame
        let headerBefore = try XCTUnwrap(try app.wait(recipients).frame)
        // Dragged from the recipients line, not from the centre of the screen. On this message the
        // centre lands inside the invitation card, on the "Around this meeting" strip, and that
        // view answers a vertical drag itself: the gesture is consumed, nothing scrolls, and the
        // test then reports a header that stayed put as if the header were the thing at fault.
        //
        // The line rather than a fraction of the screen, because a fraction is a guess about where
        // this fixture's card ends on a device nobody has run yet. This is static text inside the
        // content that scrolls, so it consumes nothing and it is where the reader's thumb would be.
        let deadline = Date().addingTimeInterval(ShowcaseApp.timeout)
        var scrolled = false
        repeat {
            try app.point(app.wait(recipients)).press(
                forDuration: 0.05,
                thenDragTo: app.input.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.05))
            )
            try app.settle()
            // A header that has scrolled clean off the top is the expected outcome, and the agent
            // leaves what is off screen out.
            let header = try app.query(recipients).first?.frame
            scrolled = header.map { $0.minY < headerBefore.minY } ?? true
        } while !scrolled && Date() < deadline

        XCTAssertEqual(
            try app.wait(reply).frame, rowBefore,
            "the action row moved with the message instead of staying put"
        )
        XCTAssertTrue(
            scrolled, "the reading header stayed put while the message scrolled under it\n\(app.dump())"
        )
    }
}
