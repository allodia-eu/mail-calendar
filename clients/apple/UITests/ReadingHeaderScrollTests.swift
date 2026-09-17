// The reading header rides the message's scroll, and the action row does not
// (`docs/reading-actions.md`).
//
// Nothing below the UI can see this. The header travels inside the scroll view the body brings,
// on the `contentInset` the reading pane gives it, so whether it actually moves is a question
// about two live views agreeing, which only a running screen answers.

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
    func testTheHeaderScrollsAwayAndTheActionRowStays() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.openMessage(containing: "kickoff", in: app)

        let reply = app.buttons["Reply"]
        XCTAssertTrue(
            reply.waitForExistence(timeout: ShowcaseApp.timeout),
            "no message was opened, so there is no reading view to scroll"
        )
        // The recipients line, which every message in the dataset has and no other surface draws
        // while a message is open.
        let recipients = app.staticTexts["To:"]
        XCTAssertTrue(
            recipients.waitForExistence(timeout: ShowcaseApp.timeout),
            "the reading header drew no recipients line to follow"
        )

        let rowBefore = reply.frame
        let headerBefore = recipients.frame
        // ⚠️ **The drag is repeated until the message answers it**, because the header is on
        // screen a good while before the scroll it rides exists. The header is a subview of the
        // body's scroll view, and that view has nothing to scroll until WebKit has committed the
        // document: its content is shorter than the pane until then, so the drag rubber-bands back
        // and leaves the header where it was. Nothing on screen marks that moment, the header is
        // drawn and addressable from the first layout pass, and the wait is long enough to lose
        // on: two and a half seconds per message on an idle simulator, against the four the
        // queries above cost. One drag therefore reports the race as a header that will not move,
        // which is what turned this red on a hosted runner while every local run passed.
        let deadline = Date().addingTimeInterval(ShowcaseApp.timeout)
        var scrolled = false
        repeat {
            // Dragged from the recipients line, NOT `app.swipeUp()`, which swipes the centre of the
            // screen. On this message the centre lands inside the invitation card, on the "Around
            // this meeting" strip, and that view answers a vertical drag itself: the gesture is
            // consumed, nothing scrolls, and the test then reports a header that stayed put as if
            // the header were the thing at fault. Measured on an iPhone Air simulator, a centre
            // swipe scrolled nothing in five runs of fourteen, and a hosted runner failed on it
            // twice in a row; a drag from this line has not missed.
            //
            // The line rather than a fraction of the screen, because a fraction is a guess about
            // where this fixture's card ends on a device nobody has run yet. This is static text
            // inside the content that scrolls, so it consumes nothing and it is where the reader's
            // thumb would be.
            recipients.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
                .press(
                    forDuration: 0.05,
                    thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.05))
                )
            // `exists` is checked first: a header that has scrolled clean off the top is the
            // expected outcome, and reading `frame` off an element that is gone answers zero,
            // which would pass the comparison for the wrong reason.
            scrolled = !recipients.exists || recipients.frame.minY < headerBefore.minY
        } while !scrolled && Date() < deadline

        XCTAssertEqual(
            reply.frame, rowBefore,
            "the action row moved with the message instead of staying put"
        )
        XCTAssertTrue(scrolled, "the reading header stayed put while the message scrolled under it")
    }
}
