// The composer's presentation lifecycle.
//
// Its own file rather than a second case in `NavigationBarTests`, because it is the slowest test
// here by some way (a full-screen cover presented, dismissed and presented again, at animation
// speed) and CI runs the suite by class. What it costs is worth paying locally and, for now, not
// on every pull request.
//
// `SheetItemBinding` exists for exactly the bug this observes: a binding that is not cleared when a
// presentation goes away leaves the control looking fine and refusing to work a second time.
// Nothing below the UI can see it, so until this file there was no gate on it at all.

import XCTest

@MainActor
final class ComposerPresentationTests: XCTestCase {
    func testComposeOpensCancelsAndOpensAgain() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)

        ShowcaseApp.tap(app.buttons["Compose"])
        XCTAssertTrue(
            app.navigationBars["New Message"].waitForExistence(timeout: ShowcaseApp.timeout),
            "Compose did not open the composer"
        )
        XCTAssertFalse(app.buttons["Send"].isEnabled, "an empty composer offered Send")

        ShowcaseApp.tap(app.buttons["Cancel"])
        XCTAssertTrue(
            app.buttons["Compose"].waitForExistence(timeout: ShowcaseApp.timeout),
            "Cancel did not return to the mailbox"
        )

        // The second time is the assertion. A binding that is cleared on dismissal and one that is
        // not look identical until the control is used twice.
        ShowcaseApp.tap(app.buttons["Compose"])
        XCTAssertTrue(
            app.navigationBars["New Message"].waitForExistence(timeout: ShowcaseApp.timeout),
            "Compose opened once and then stopped opening"
        )
    }
}
