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
        // Waited for: after a switch from another tab the field arrives a moment after the buttons.
        XCTAssertTrue(
            app.searchFields["Search mail"].waitForExistence(timeout: ShowcaseApp.timeout),
            "the mailbox bar has no search field"
        )
    }

    /// An open message has the screen to itself: the tab bar goes while it is read, and comes back
    /// with the list. Calendar stands for the bar, being a tab and nothing else on either screen.
    func testAnOpenMessageHidesTheTabBarAndBackBringsItBack() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.openFirstMessage(app)
        XCTAssertTrue(
            app.buttons["Reply"].waitForExistence(timeout: ShowcaseApp.timeout),
            "no message was opened"
        )
        let calendar = app.buttons["Calendar"]
        let gone = NSPredicate(format: "exists == false OR isHittable == false")
        wait(for: [expectation(for: gone, evaluatedWith: calendar)], timeout: ShowcaseApp.timeout)

        app.navigationBars.buttons.element(boundBy: 0).tap()
        XCTAssertTrue(
            calendar.waitForExistence(timeout: ShowcaseApp.timeout) && calendar.isHittable,
            "the tab bar did not come back with the list"
        )
    }
}
