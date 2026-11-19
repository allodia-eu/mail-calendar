// The calendar screen's half of the pane beside it (`docs/calendar-pane.md`): where the grid rests,
// for the pane to ask about, and the seat a pressed mini-month day asks the grid to take.

import MailcalBindings
import SwiftUI

extension CalendarScreenView {
    /// Where the grid rests: the first shown day for a time grid (it changes only when the strip comes
    /// to rest on another day), the month's anchor for the month, and the shape alone for the agenda.
    func paneGrid(_ pager: CalendarPager, _ calendar: Calendar) -> CalendarPaneGrid {
        let mode = pager.mode
        let first = mode.isGrid
            ? calendar.date(byAdding: .day, value: position.restingDay, to: pager.anchor(forPage: 0))
            : pager.anchor(forPage: page)
        return CalendarPaneGrid(
            layout: mode.layout,
            anchor: isoDate(first ?? pager.origin, calendar: calendar),
            columns: mode.columns
        )
    }

    /// Takes the layout the core gave a pressed day, through the path the layout menu uses, seated
    /// on the pressed day, which `seatCalendar` has made the focus, and frames the grid on it.
    func applySeat(_ request: CalendarSeatRequest?, calendar: Calendar) {
        guard
            let seat = request?.seat,
            let day = model.calendarFocus.flatMap({ parseISODate($0, calendar: calendar) })
        else { return }
        guard var current = pager else { return }
        current.focus(on: day, weekStart: model.weekStart(of:), core: model)
        pager = current
        setMode(seat.layout.mode, calendar: calendar)
    }
}
