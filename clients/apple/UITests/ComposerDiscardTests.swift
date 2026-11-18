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
    func testDiscardingAWrittenMessageClosesTheComposer() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)
        try app.press(.button("New Mail"))
        let subject = Filter.text("Subject").role("AXTextField")
        try app.fill(subject, "Lunch on Friday")

        // The iPad's page sheet leaves the mailbox's own More beside it; the composer's is the one
        // across the sheet's width.
        let sheet = try app.wait(subject).frame ?? .null
        let more = try app.query(.button("More actions"))
            .first { ($0.frame?.midX ?? -1) > sheet.minX && ($0.frame?.midX ?? -1) < sheet.maxX + 120 }
        let composerMore = try XCTUnwrap(more, "the composer has no More")
        try app.activate(composerMore)
        try app.press(.button("Discard"))
        // The alert's Discard, which stands alone once the menu that held the first has gone.
        try app.press(
            .button("Discard"), "a written message was discarded without asking"
        )

        try app.wait(.button("New Mail"), "Discard did not close the composer")
        try app.waitGone(subject, "the composer is still open behind the mailbox")
    }
}
