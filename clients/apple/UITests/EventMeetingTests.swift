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

    /// Switches the calendar to the Agenda: a segment of the view picker where all six fit; an
    /// item of the picker as a menu, named for the view on show, on an upright iPad beside its
    /// sidebar; and on a phone an item in the toolbar's More actions menu.
    private func showAgenda(_ app: App) throws {
        // New Event, the calendar's own: the reading pane beside the mailbox has a More actions
        // too, and the launch shows the mailbox before the hook brings the calendar.
        try app.wait(.button("New Event"), "the calendar did not come up")
        // Which shape the picker took is read once the arriving calendar has stopped moving.
        try app.settle()
        if try app.exists(.button("Agenda")) {
            try app.press(.button("Agenda"))
            return
        }
        // By its start: the match is in any case, and "Design review, …" is an event.
        let picker = try app.query(.textLike("View, ").role("AXButton"))
            .first { $0.label.hasPrefix("View, ") }
        let menu = try picker ?? app.wait(.button("More actions"))
        try app.choose(.button("Agenda"), from: menu)
    }
}
