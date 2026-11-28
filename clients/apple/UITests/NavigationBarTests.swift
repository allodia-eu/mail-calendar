// The mailbox's navigation bar.
//
// `idb` reports the whole top bar as a single unlabelled `AXGroup`, so the exploratory tooling
// cannot reach Compose, More, Select or Cancel there (`.agents/skills/debug-app`). The agent reads
// the app's own accessibility objects, with UIKit's names for its own controls loaded, and gets
// every item with its label.

import UIKit
import XCTest

@MainActor
final class NavigationBarTests: XCTestCase {
    /// Every item the mailbox's bar carries, reachable by its accessible name.
    ///
    /// A pixel tap would pass over a bar whose buttons had lost their labels, which is the failure
    /// a screen-reader user meets first.
    ///
    /// There is no overflow menu here, and its absence is the design rather than a gap: syncing
    /// was the only thing behind it, and on a phone that is the pull the list already answers.
    /// The reading view's own row is where an overflow belongs (`docs/reading-actions.md`), and
    /// `ReadingActionRowTests` is what holds it there.
    func testMailboxBarItemsAreReachableByName() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)

        for name in [ShowcaseApp.folders, "Select", "New Mail"] {
            XCTAssertTrue(
                try ShowcaseApp.reachable(name, in: app), "the mailbox bar has no reachable \(name)"
            )
        }
        // Waited for: after a switch from another tab the field arrives a moment after the buttons.
        try app.wait(.text("Search mail").role("AXSearchField"), "the mailbox bar has no search field")
    }

    /// The tab bar stays while a message is open, and the Mail tab keeps the message: checking the
    /// calendar against a time someone proposed is a round trip that ends on the message, not on
    /// the list it was opened from.
    func testTheTabBarStaysOverAMessageAndMailKeepsItAcrossAVisitToTheCalendar() throws {
        try XCTSkipIf(
            ShowcaseApp.pad,
            "the iPad has no tab bar, and reads beside the list"
        )
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.openFirstMessage(app)
        try app.wait(.button("Reply"), "no message was opened")

        try ShowcaseApp.tab("Calendar", in: app)
        try app.waitGone(.button("Reply"), timeout: ShowcaseApp.timeout, "the calendar did not come up")
        try ShowcaseApp.tab("Mail", in: app)
        try app.wait(.button("Reply"), "the Mail tab came back to the list, not to the open message")
    }
}
