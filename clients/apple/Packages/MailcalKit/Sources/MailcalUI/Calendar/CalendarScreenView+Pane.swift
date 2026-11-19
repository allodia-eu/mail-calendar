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

    /// Takes the seat the core gave a pressed day, through the path the layout menu uses, then frames
    /// the grid on that day rather than on today.
    func applySeat(_ request: CalendarSeatRequest?, calendar: Calendar) {
        guard let seat = request?.seat, let day = parseISODate(seat.anchor, calendar: calendar) else {
            return
        }
        let mode = seat.layout.mode
        setMode(mode, calendar: calendar)
        guard var current = pager else { return }
        current.jump(to: mode.isMonth ? day : model.weekStart(of: day))
        pager = current
        page = 0
        seatDay = day
        recentreToken += 1
    }
}
