// Discarding a written message closes the composer on iPhone and iPad (docs/drafts.md).
//
// Something written means Discard asks first, and the composer is then closed from inside that
// "Discard draft?" alert, which is the step a host can lose: on the Mac a window with the alert
// still attached ignores being closed. That window is outside this suite, which runs only on iOS;
// it is checked by hand with `scripts/dev/control.sh macos agent` (open in a new window, write a
// subject, Save as draft, Discard, Discard), and that nothing comes back to Drafts is the core's
// `a_save_after_the_discard_stores_nothing`.

import XCTest

@MainActor
final class ComposerDiscardTests: XCTestCase {
    func testDiscardingAWrittenMessageClosesTheComposer() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)
        ShowcaseApp.tap(app.buttons["New Mail"])
        let subject = app.textFields["Subject"]
        XCTAssertTrue(subject.waitForExistence(timeout: ShowcaseApp.timeout), "no composer")
        ShowcaseApp.tap(subject)
        subject.typeText("Lunch on Friday")

        // The iPad's page sheet leaves the mailbox's own More beside it; the composer's is the one
        // across the sheet's width.
        let sheet = subject.frame
        let more = app.buttons.matching(identifier: "More actions").allElementsBoundByIndex
            .first { $0.frame.midX > sheet.minX && $0.frame.midX < sheet.maxX + 120 }
        XCTAssertNotNil(more, "the composer has no More")
        if let more { ShowcaseApp.tap(more) }
        ShowcaseApp.tap(app.buttons["Discard"])
        let confirm = app.alerts.buttons["Discard"]
        XCTAssertTrue(
            confirm.waitForExistence(timeout: ShowcaseApp.timeout),
            "a written message was discarded without asking"
        )
        confirm.tap()

        XCTAssertTrue(
            app.buttons["New Mail"].waitForExistence(timeout: ShowcaseApp.timeout),
            "Discard did not close the composer"
        )
        XCTAssertFalse(subject.exists, "the composer is still open behind the mailbox")
    }
}
