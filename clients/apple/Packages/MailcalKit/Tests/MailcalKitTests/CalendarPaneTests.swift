// The calendar pane's one piece of client state: the paged month, and when it goes.

import MailcalBindings
@testable import MailcalUI
import XCTest

final class CalendarPaneTests: XCTestCase {
    private let week = CalendarPaneGrid(layout: .week, anchor: "2026-11-16", columns: 7)

    func testBeforeTheGridHasDrawnThePaneAsksAboutToday() {
        let query = CalendarPaneModel().query(today: "2026-11-18")
        XCTAssertEqual(query.anchor, "2026-11-18")
        XCTAssertEqual(query.layout, .week)
        XCTAssertNil(query.month)
    }

    func testThePaneAsksAboutWhereTheGridRests() {
        var model = CalendarPaneModel()
        model.follow(CalendarPaneGrid(layout: .threeDay, anchor: "2026-11-19", columns: 3))
        let query = model.query(today: "2026-11-18")
        XCTAssertEqual(query.layout, .threeDay)
        XCTAssertEqual(query.anchor, "2026-11-19")
        XCTAssertEqual(query.columns, 3)
    }

    func testAPagedMonthHoldsWhileTheGridStaysPut() {
        var model = CalendarPaneModel()
        model.follow(week)
        model.page(to: "2027-01-01")
        model.follow(week)
        XCTAssertEqual(model.query(today: "2026-11-18").month, "2027-01-01")
    }

    func testAMovedGridTakesThePagedMonthAway() {
        var model = CalendarPaneModel()
        model.follow(week)
        model.page(to: "2027-01-01")
        model.follow(CalendarPaneGrid(layout: .week, anchor: "2026-11-23", columns: 7))
        XCTAssertNil(model.query(today: "2026-11-18").month)
    }

    func testASeatRequestIsNewEachTimeADayIsPressed() {
        let seat = CalendarSeat(layout: .day, anchor: "2026-11-18")
        XCTAssertNotEqual(CalendarSeatRequest(seat: seat), CalendarSeatRequest(seat: seat))
    }
}
