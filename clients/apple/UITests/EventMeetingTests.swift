// An event's video meeting in its detail (docs/conferencing.md).
//
// The showcase's Monday design sync carries a Teams meeting in every locale. The test reaches it
// through the Agenda, which lists the whole week whatever day it runs on; the grid shows only the
// days around today, so a Saturday run would never see Monday's block.

import XCTest

@MainActor
final class EventMeetingTests: XCTestCase {
    /// The detail of an event with a meeting offers Join, under the service's name, with the
    /// meeting ID beside it.
    func testAMeetingOpensWithJoin() {
        let app = ShowcaseApp.launch(["MAILCAL_CALENDAR": "1"])
        showAgenda(app)

        let sync = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label BEGINSWITH %@", "Design sync"))
            .firstMatch
        XCTAssertTrue(sync.waitForExistence(timeout: ShowcaseApp.timeout), "the agenda did not list the design sync")
        XCTAssertTrue(
            sync.value as? String == "Video meeting",
            "the agenda row did not say the event has a video meeting"
        )
        ShowcaseApp.tap(sync)

        XCTAssertTrue(
            app.buttons["event-join"].waitForExistence(timeout: ShowcaseApp.timeout),
            "the detail offered no Join"
        )
        XCTAssertTrue(app.staticTexts["Microsoft Teams"].exists, "the meeting is not headed by its service")
        XCTAssertTrue(app.staticTexts["990 000 000 001"].exists, "the meeting ID is missing")
    }

    /// Switches the calendar to the Agenda: a segment of the view picker where there is room for
    /// one, and otherwise an item in the toolbar's More actions menu.
    private func showAgenda(_ app: XCUIApplication) {
        let segment = app.buttons["Agenda"]
        if !segment.waitForExistence(timeout: ShowcaseApp.timeout) {
            ShowcaseApp.tap(app.buttons["More actions"])
        }
        ShowcaseApp.tap(app.buttons["Agenda"])
    }
}
