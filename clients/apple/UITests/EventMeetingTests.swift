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
    func testAMeetingOpensWithJoin() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_CALENDAR": "1"])
        try showAgenda(app)

        let sync = try app.wait(.textLike("Design sync"), "the agenda did not list the design sync")
        XCTAssertEqual(
            sync.value, "Video meeting", "the agenda row did not say the event has a video meeting"
        )
        try app.tap(sync)

        try app.wait(.id("event-join"), "the detail offered no Join")
        XCTAssertTrue(
            try app.exists(.text("Microsoft Teams")), "the meeting is not headed by its service"
        )
        XCTAssertTrue(try app.exists(.text("990 000 000 001")), "the meeting ID is missing")
    }

    /// Switches the calendar to the Agenda: a segment of the view picker where there is room for
    /// one, and otherwise an item in the toolbar's More actions menu.
    private func showAgenda(_ app: App) throws {
        try app.wait(.button("More actions"), "the calendar did not come up")
        if try !app.exists(.button("Agenda")) {
            try app.press(.button("More actions"))
        }
        try app.press(.button("Agenda"))
    }
}
