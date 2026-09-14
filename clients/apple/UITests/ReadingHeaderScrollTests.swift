// The reading header rides the message's scroll, and the action row does not
// (`docs/reading-actions.md`).
//
// Nothing below the UI can see this. The header is a SwiftUI view drawn *over* the body and moved
// by the offset the body's scroll view reports, so whether it actually moves is a question about
// two live views agreeing, which only a running screen answers.

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
        app.swipeUp()

        XCTAssertEqual(
            reply.frame, rowBefore,
            "the action row moved with the message instead of staying put"
        )
        // `exists` is checked first: a header that has scrolled clean off the top is the expected
        // outcome, and reading `frame` off an element that is gone answers zero, which would pass
        // the comparison below for the wrong reason.
        XCTAssertTrue(
            !recipients.exists || recipients.frame.minY < headerBefore.minY,
            "the reading header stayed put while the message scrolled under it"
        )
    }
}
