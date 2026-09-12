// The calendar's navigation model: which week a page shows, and what a zoom does to it.
//
// The rule these tests exist to defend: **a zoom must not move the days**. A page is a whole week and
// the zoom only decides how many of its columns fit on screen. The first Android design snapped a
// pinch to a differently-anchored "view", and a Monday-aligned week cannot contain an arbitrary
// three-day window, so a user reading Sunday-to-Tuesday who pinched outwards was shown the *previous*
// Monday-to-Sunday, and two of the three days they were reading vanished. It looked like a glitch; it
// was the design. It must not be re-introduced here.

import Foundation
import Testing

@testable import MailcalUI

/// A fixed calendar in a fixed zone, so nothing here depends on where the machine is.
private func calendar() -> Calendar {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(identifier: "Europe/Amsterdam")!
    calendar.locale = Locale(identifier: "en_GB")
    return calendar
}

private func date(_ iso: String) -> Date {
    parseISODate(iso, calendar: calendar())!
}

/// The Monday opening the week that holds Sunday 2026-07-12.
private let weekStart = "2026-07-06"

@Suite struct CalendarPagerTests {

    @Test func aGridPageIsAWholeWeekAndSwipingMovesAWholeWeek() {
        let pager = CalendarPager(origin: date(weekStart), mode: .threeDay, calendar: calendar())
        #expect(pager.anchor(forPage: 0) == date(weekStart))
        #expect(pager.anchor(forPage: 1) == date("2026-07-13"))
        #expect(pager.anchor(forPage: -1) == date("2026-06-29"))
    }

    @Test func thePageIsTheSameWeekAtEveryZoom() throws {
        // The whole point. Day, three-day, work-week and week are ONE grid at four zooms, the page
        // they sit on does not change, so neither can the days.
        for mode in CalendarMode.gridModes {
            let pager = CalendarPager(origin: date(weekStart), mode: mode, calendar: calendar())
            #expect(pager.anchor(forPage: 0) == date(weekStart), "\(mode)")
            #expect(pager.anchor(forPage: 2) == date("2026-07-20"), "\(mode)")
        }
    }

    @Test func aZoomDoesNotMoveTheWeek() {
        // THE regression. `setZoom` leaves the origin alone: after a pinch the user is looking at the
        // same week, with the columns a different width. `setMode` (a menu choice) may re-origin; a
        // pinch may not.
        var pager = CalendarPager(origin: date(weekStart), mode: .threeDay, calendar: calendar())
        let week = pager.anchor(forPage: 3)

        pager.setZoom(.week)
        #expect(pager.mode == .week)
        #expect(pager.origin == date(weekStart), "the origin moved under a zoom")
        #expect(pager.anchor(forPage: 3) == week, "the week on screen changed under a zoom")

        pager.setZoom(.day)
        #expect(pager.anchor(forPage: 3) == week)
    }

    @Test func aZoomCannotTurnTheGridIntoAMonthOrAnAgenda() {
        // Those are different layouts, not zoom levels, a pinch must not be able to reach them.
        var pager = CalendarPager(origin: date(weekStart), mode: .week, calendar: calendar())
        pager.setZoom(.month)
        #expect(pager.mode == .week)
        pager.setZoom(.agenda)
        #expect(pager.mode == .week)
    }

    @Test func choosingAShapeFromTheMenuKeepsThePeriodYouAreLookingAt() {
        var pager = CalendarPager(origin: date(weekStart), mode: .week, calendar: calendar())
        let week = pager.anchor(forPage: 3)
        pager.setMode(.month, currentPage: 3)
        #expect(pager.origin == week)
    }

    @Test func aMonthPagesByCalendarMonthNotByAFixedNumberOfDays() {
        // Months are 28–31 days long. Striding by any constant would drift off the month within a
        // year, silently, because each page still renders a perfectly plausible grid.
        let pager = CalendarPager(origin: date("2026-07-15"), mode: .month, calendar: calendar())
        #expect(pager.anchor(forPage: 0) == date("2026-07-01"))
        #expect(pager.anchor(forPage: 1) == date("2026-08-01"))
        #expect(pager.anchor(forPage: -1) == date("2026-06-01"))
        // Twelve pages on is exactly a year, whatever the month lengths in between.
        #expect(pager.anchor(forPage: 12) == date("2027-07-01"))
    }

    @Test func pagingFromTheEndOfALongMonthDoesNotLoseADayEachTime() {
        // The trap: adding a month to the 31st clamps to the 28th in February, and if the *next*
        // page were measured from the clamped date, the anchor would walk backwards through the year.
        // Anchoring on the 1st is what stops it.
        let pager = CalendarPager(origin: date("2026-01-31"), mode: .month, calendar: calendar())
        #expect(pager.anchor(forPage: 0) == date("2026-01-01"))
        #expect(pager.anchor(forPage: 1) == date("2026-02-01"))
        #expect(pager.anchor(forPage: 2) == date("2026-03-01"))
        #expect(pager.anchor(forPage: 3) == date("2026-04-01"))
    }

    @Test func eachShapeShowsTheRightNumberOfColumns() {
        #expect(CalendarMode.day.columns == 1)
        #expect(CalendarMode.threeDay.columns == 3)
        #expect(CalendarMode.workWeek.columns == 5)
        #expect(CalendarMode.week.columns == 7)

        #expect(modeForColumns(1) == .day)
        #expect(modeForColumns(3) == .threeDay)
        #expect(modeForColumns(5) == .workWeek)
        #expect(modeForColumns(7) == .week)
        // A settled pinch often lands between two rungs. The tie breaks towards MORE days: showing a
        // day the user did not ask for is a smaller sin than hiding one they did.
        #expect(modeForColumns(2) == .threeDay)
        #expect(modeForColumns(6) == .week)
    }

    @Test func theAgendaAndTheMonthAreNotTheTimeGrid() {
        #expect(!CalendarMode.agenda.isGrid)
        #expect(!CalendarMode.month.isGrid)
        #expect(CalendarMode.month.isMonth)
        #expect(CalendarMode.week.isGrid)
    }
}
