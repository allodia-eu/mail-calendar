// The composer's presentation lifecycle.
//
// Its own file rather than a second case in `NavigationBarTests`, because CI runs the suite by
// class and this one presents a full-screen cover, dismisses it and presents it again, at animation
// speed.
//
// `SheetItemBinding` exists for exactly the bug this observes: a binding that is not cleared when a
// presentation goes away leaves the control looking fine and refusing to work a second time.
// Nothing below the UI can see it.

import XCTest

@MainActor
final class ComposerPresentationTests: XCTestCase {
    func testComposeOpensCancelsAndOpensAgain() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)

        try app.press(.button("New Mail"))
        let send = try app.wait(.button("Send"), "Compose did not open the composer")
        XCTAssertFalse(send.enabled, "an empty composer offered Send")

        try app.press(.button("Close"))
        try app.wait(.button("New Mail"), "Close did not return to the mailbox")

        // The second time is the assertion. A binding that is cleared on dismissal and one that is
        // not look identical until the control is used twice.
        try app.press(.button("New Mail"))
        try app.wait(.button("Send"), "Compose opened once and then stopped opening")
    }
}
