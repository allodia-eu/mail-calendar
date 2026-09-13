// The mailbox's navigation bar.
//
// Why this is the first thing the suite asserts: on iOS the navigation bar is the one part of the
// screen the exploratory tooling cannot read. `idb` reports the whole top bar as a single
// unlabelled `AXGroup` and stops identically in Apple's own Settings app, so `control.sh find` and
// `press` cannot reach Compose, More, Select or Cancel and the only way through it is to guess a
// pixel (`.agents/skills/debug-app`). XCUITest asks the app in-process and gets every item with its
// label, so what was unreachable there is ordinary here.

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
    func testMailboxBarItemsAreReachableByName() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)

        for name in ["Folders", "Select", "New Mail"] {
            XCTAssertTrue(
                ShowcaseApp.reachable(name, in: app),
                "the mailbox bar has no reachable \(name)"
            )
        }
        XCTAssertTrue(app.searchFields["Search mail"].exists, "the mailbox bar has no search field")
    }
}
