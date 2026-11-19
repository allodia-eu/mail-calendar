// The calendar screen's moves around the focus date (`docs/calendar.md`, "The focus date"): every
// view is seated on the core's focus, previous and next move it a period, and a pressed day puts it
// somewhere. The rules are the core's; this only asks and draws what it answers.

import MailcalBindings
import SwiftUI

extension CalendarScreenView {
    /// A pager in `mode`, seated on `focus`.
    func seatedPager(_ mode: CalendarMode, focus: Date, calendar: Calendar) -> CalendarPager {
        CalendarPager(
            origin: mode.isGrid ? model.weekStart(of: focus) : focus,
            mode: mode, calendar: calendar, focus: focus
        )
    }

    /// A week along the strip, a month of the month grid; nothing in the agenda, which has no period.
    ///
    /// The grid's focus is not moved here: it follows the strip when the strip comes to rest
    /// (`settleFocus`), which a scroll and a flick reach as well as a chevron.
    func step(_ direction: Int, mode: CalendarMode, today: Date) {
        if mode.isGrid {
            // From wherever the strip is, so a chevron pressed mid-seam keeps the day it is framed
            // on rather than re-aligning the week under the user.
            position.held = true
            withAnimation(.easeOut(duration: 0.25)) { position.strip.weeks += CGFloat(direction) }
        } else if mode.isMonth, var current = pager {
            current.stepMonth(direction, today: today, core: model)
            pager = current
            page = 0
        }
    }

    /// The strip came to rest on `day`, counted from the origin week's first day.
    func settleFocus(restingDay day: Int, today: Date) {
        guard var current = pager else { return }
        let week = Int((Double(day) / Double(daysInWeek)).rounded(.down))
        current.settle(week: week, today: today, core: model)
        pager = current
    }

    /// A shape from the menu: the new view is seated on the focus, which does not move.
    func setMode(_ next: CalendarMode, calendar: Calendar) {
        guard var current = pager else { return }
        model.setCalendarLayout(next.layout)
        current.setMode(next, weekStart: model.weekStart(of:))
        // Picking a shape from the MENU is where week alignment happens, not on a zoom, which must
        // leave the days exactly where they are.
        if next.isGrid { position.zoom.resetDays(next.columns) }
        pager = current
        page = 0
        seatDay = current.focus
        recentreToken += 1
    }

    func backToToday(today: Date) {
        guard var current = pager else { return }
        current.focus(on: today, weekStart: model.weekStart(of:), core: model)
        pager = current
        seatDay = nil
        page = 0
        // The strip runs on for ever, so a jump home is a real journey: `recentre` frames it, and
        // landing on the right week while looking at the wrong end of it is only half a jump.
        recentreToken += 1
    }

    /// A month day's own space was pressed: the focus goes there, and the month stays as it is.
    func pickDay(_ date: String, calendar: Calendar) {
        guard var current = pager, let day = parseISODate(date, calendar: calendar) else { return }
        current.pick(day, core: model)
        pager = current
    }
}
