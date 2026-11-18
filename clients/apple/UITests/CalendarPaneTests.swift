// The calendar pane (`docs/calendar-pane.md`): on an iPad at regular width the pane beside the
// calendar is the mini month and the calendars; on an iPhone there is no pane and the manager stays.
//
// What the core decides about the pane is tested in Rust. These are the parts only a running screen
// shows: that the pane is there at all, that a row and a day reach the grid, and that Mail is a way
// back.

import UIKit
import XCTest

@MainActor
final class CalendarPaneTests: XCTestCase {
    private var isPad: Bool { UIDevice.current.userInterfaceIdiom == .pad }

    private let monthTitle = Filter.id("CalendarPaneMonthTitle")

    func testAnIPadShowsThePaneAndAnIPhoneKeepsTheManager() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_CALENDAR": "1"])
        if isPad {
            try app.wait(monthTitle, "the calendar pane did not come up beside the calendar")
            XCTAssertTrue(try app.exists(.textLike("Work, ")), "the Work calendar is not listed")
        } else {
            try app.wait(.button("More actions"), "the calendar toolbar did not come up")
            XCTAssertFalse(try app.exists(monthTitle), "a phone drew the pane")
            try app.press(.button("More actions"))
            try app.wait(.button("Manage calendars"), "the phone lost its calendar manager")
        }
    }

    func testUntickingACalendarTakesItsEventsOffTheGrid() throws {
        try XCTSkipUnless(isPad, "only an iPad has the pane")
        let app = try ShowcaseApp.launch(["MAILCAL_CALENDAR": "1"])
        let gym = Filter.textLike("Gym, ").hidden()
        try app.wait(gym, "the Personal calendar's gym session is not on the grid")

        try app.press(.textLike("Personal, "))
        XCTAssertTrue(try app.goes(gym), "unticking Personal left its events on the grid")

        try app.press(.textLike("Personal, "))
        try app.wait(gym, "ticking Personal again did not bring its events back")
    }

    func testPressingADayMovesTheGrid() throws {
        try XCTSkipUnless(isPad, "only an iPad has the pane")
        let app = try ShowcaseApp.launch(["MAILCAL_CALENDAR": "1"])
        try app.wait(monthTitle, "the calendar pane did not come up")

        // The 15th of next month is in no week the grid opens on, whatever today is.
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "Europe/Amsterdam") ?? .current
        let next = try XCTUnwrap(calendar.date(byAdding: .month, value: 1, to: Date()))
        var parts = calendar.dateComponents([.year, .month], from: next)
        parts.day = 15
        let day = try XCTUnwrap(calendar.date(from: parts))
        let formatter = DateFormatter()
        formatter.calendar = calendar
        formatter.timeZone = calendar.timeZone
        formatter.dateFormat = "yyyy-MM-dd"
        let id = "CalendarPaneDay_\(formatter.string(from: day))"

        try app.press(.id("CalendarPaneNext"))
        try app.press(.id(id))
        XCTAssertTrue(
            try eventually {
                try app.query(.id(id)).first?.label.contains("Shown in the calendar") == true
            },
            "the grid did not move to the pressed day"
        )
    }

    func testMailReturnsToTheMailList() throws {
        try XCTSkipUnless(isPad, "only an iPad has the pane")
        let app = try ShowcaseApp.launch(["MAILCAL_CALENDAR": "1"])
        try app.wait(monthTitle, "the calendar pane did not come up")

        try app.press(.id("NavMail"))
        try app.wait(.text("Search mail").role("AXSearchField"), "Mail did not bring the mailbox back")
        XCTAssertFalse(try app.exists(monthTitle), "the calendar pane stayed beside the mailbox")
    }

    /// Polls `condition` until it holds, for a state no single filter names: one day's label among
    /// the band's seven.
    private func eventually(_ condition: () throws -> Bool) throws -> Bool {
        let deadline = Date().addingTimeInterval(ShowcaseApp.timeout)
        while Date() < deadline {
            if try condition() { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.25))
        }
        return try condition()
    }
}
