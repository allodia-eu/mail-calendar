// The calendar's focus date (`docs/calendar.md`, "The focus date"): every view is seated on it,
// switching views never moves it, and previous and next move it a period.
//
// The stepping rules (the same day of the month, a short month's last day, a step into today's
// week or month landing on today) are the core's and are tested there. These defend the client's
// half: that every move goes through the core and every view is drawn around what it answers.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

private func calendar() -> Calendar {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(identifier: "Europe/Amsterdam")!
    calendar.locale = Locale(identifier: "en_GB")
    return calendar
}

private func date(_ iso: String) -> Date {
    parseISODate(iso, calendar: calendar())!
}

/// Monday-first weeks, as the core would answer for a Monday-first setting.
private func weekStart(_ day: Date) -> Date {
    let calendar = calendar()
    let weekday = (calendar.component(.weekday, from: day) + 5) % 7
    return calendar.date(byAdding: .day, value: -weekday, to: day)!
}

/// A stand-in for the core that only shifts the date, and records what it was asked.
@MainActor
private final class FakeFocus: CalendarFocusing {
    var focus: Date
    var steps: [(CalendarLayout, Int)] = []

    init(_ focus: Date) { self.focus = focus }

    func setCalendarFocus(_ date: Date) { focus = date }

    func stepCalendarFocus(_ layout: CalendarLayout, _ steps: Int, today: Date) -> Date {
        self.steps.append((layout, steps))
        let unit: Calendar.Component = layout == .month ? .month : .weekOfYear
        focus = calendar().date(byAdding: unit, value: steps, to: focus)!
        return focus
    }
}

@MainActor
@Suite struct CalendarFocusTests {
    private let today = date("2026-11-19")

    private func pager(_ mode: CalendarMode, focus: Date) -> CalendarPager {
        CalendarPager(
            origin: mode.isGrid ? weekStart(focus) : focus, mode: mode, calendar: calendar(),
            focus: focus
        )
    }

    @Test func weekThenMonthThenWeekComesBackToTheSameWeek() {
        let core = FakeFocus(date("2026-09-16"))
        var pager = pager(.week, focus: core.focus)
        pager.setMode(.month, weekStart: weekStart)
        #expect(pager.anchor(forPage: 0) == date("2026-09-01"))
        pager.setMode(.week, weekStart: weekStart)
        #expect(pager.origin == date("2026-09-14"))
        #expect(pager.focus == date("2026-09-16"))
        #expect(core.steps.isEmpty, "switching views moved the focus: \(core.steps)")
    }

    @Test func aMonthPagedOnOpensTheWeekHoldingTheFocusNotTheFirst() {
        // The 13th of September, a month on, is the 13th of October: the week of the 13th, not the
        // week holding the 1st, which is where Apple used to seat it.
        let core = FakeFocus(date("2026-09-13"))
        var pager = pager(.month, focus: core.focus)
        pager.stepMonth(1, today: today, core: core)
        #expect(pager.focus == date("2026-10-13"))
        #expect(pager.anchor(forPage: 0) == date("2026-10-01"))
        pager.setMode(.week, weekStart: weekStart)
        #expect(pager.origin == date("2026-10-12"))
        #expect(core.steps.count == 1 && core.steps[0].0 == .month && core.steps[0].1 == 1)
    }

    @Test func theStripComingToRestInAnotherWeekMovesTheFocusByAsManyWeeks() {
        let core = FakeFocus(date("2026-09-16"))
        var pager = pager(.threeDay, focus: core.focus)
        pager.settle(week: 0, today: today, core: core)
        #expect(core.steps.isEmpty, "resting in the focus's own week moved it")
        pager.settle(week: 2, today: today, core: core)
        pager.settle(week: 1, today: today, core: core)
        #expect(pager.focus == date("2026-09-23"))
        #expect(core.steps.map(\.1) == [2, -1])
        #expect(core.steps.allSatisfy { $0.0 == .week })
        // The month is drawn around where the strip left the focus.
        pager.setMode(.month, weekStart: weekStart)
        #expect(pager.anchor(forPage: 0) == date("2026-09-01"))
    }

    @Test func aSeatStartsCountingTheStripFromItsNewOriginWeek() {
        // A shape change re-origins the strip on the focus's week, and the strip then comes to rest
        // in week 0 of the new origin: that must not read as paging back.
        let core = FakeFocus(date("2026-09-16"))
        var pager = pager(.week, focus: core.focus)
        pager.settle(week: 3, today: today, core: core)
        pager.setMode(.day, weekStart: weekStart)
        pager.settle(week: 0, today: today, core: core)
        #expect(core.steps.map(\.1) == [3])
        #expect(pager.focus == date("2026-10-07"))
    }

    @Test func todayPutsTheFocusOnTodayAndSeatsTheViewThere() {
        let core = FakeFocus(date("2026-03-03"))
        var pager = pager(.month, focus: core.focus)
        pager.focus(on: today, weekStart: weekStart, core: core)
        #expect(core.focus == today)
        #expect(pager.anchor(forPage: 0) == date("2026-11-01"))
    }

    @Test func aPressedMonthDayTakesTheFocusAndLeavesTheMonthWhereItIs() {
        let core = FakeFocus(date("2026-09-16"))
        var pager = pager(.month, focus: core.focus)
        pager.pick(date("2026-10-02"), core: core)
        #expect(core.focus == date("2026-10-02"))
        #expect(pager.anchor(forPage: 0) == date("2026-09-01"))
        pager.setMode(.week, weekStart: weekStart)
        #expect(pager.origin == date("2026-09-28"))
    }
}
