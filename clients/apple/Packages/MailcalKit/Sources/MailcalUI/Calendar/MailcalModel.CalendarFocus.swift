// The calendar's focus date (`docs/calendar.md`, "The focus date"): the one day every view is drawn
// around, held by the core. The core signals nothing when it moves, so `calendarFocus` is this
// model's copy of it, and the month and the pane read it, so their ring follows a step at once.

import Foundation
import MailcalBindings

extension MailboxModel: CalendarFocusing {
    /// The focus as the calendar opens: today on a launch's first opening, as the core does not
    /// persist it, and wherever it was left on any opening after that.
    func openingCalendarFocus(today: Date) -> Date {
        if calendarFocus == nil { setCalendarFocus(today) }
        return calendarFocus.flatMap { parseISODate($0, calendar: gridCalendar) } ?? today
    }

    func setCalendarFocus(_ date: Date) {
        let iso = isoDate(date, calendar: gridCalendar)
        app?.setCalendarFocus(date: iso)
        calendarFocus = iso
    }

    func stepCalendarFocus(_ layout: CalendarLayout, _ steps: Int, today: Date) -> Date {
        guard let app else { return today }
        let landed = app.stepCalendarFocus(
            layout: layout, steps: Int32(clamping: steps), today: isoDate(today, calendar: gridCalendar)
        )
        calendarFocus = landed
        return parseISODate(landed, calendar: gridCalendar) ?? today
    }
}
