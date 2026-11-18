// The calendar pane's model surface (`docs/calendar-pane.md`): one pull with the grid's position
// and today as arguments, the seat a pressed mini-month day asks the calendar screen to take, and the
// group expansion the core persists.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// The pane for where the grid rests. Read from a body, so it is re-pulled whenever the calendar
    /// signals, the settings change or the pane's own inputs move; all three are observed here.
    func calendarPane() -> CalendarPane? {
        guard let app else { return nil }
        _ = calendarVersion
        _ = displaySettings
        let today = isoDate(ShowcaseMode.now ?? Date(), calendar: gridCalendar)
        return app.calendarPane(query: calendarPaneState.query(today: today))
    }

    /// A day of the mini month was pressed: asks the core where that puts the grid, for the
    /// calendar screen to apply the way its layout menu does.
    func seatCalendar(on date: String) {
        guard let app else { return }
        let layout = calendarPaneState.grid?.layout ?? calendarLayout
        calendarSeatRequest = CalendarSeatRequest(seat: app.calendarSeat(date: date, layout: layout))
    }

    /// Opens or shuts one account's group of calendars; the core persists it and signals.
    func setCalendarGroupExpanded(_ account: String, _ expanded: Bool) {
        preference(.setCalendarGroupExpanded(account: account, expanded: expanded))
    }

    /// Every calendar under its account, in the core's order: what the phone's manager groups by.
    func calendarGroups() -> [CalendarGroup] { app?.calendarGroups() ?? [] }

    /// The calendars a new event can be put on, under their accounts: Settings' default calendar
    /// and the editor's calendar picker. Read from a body, so it is re-pulled on a calendar signal.
    func newEventCalendars() -> NewEventCalendars {
        _ = calendarVersion
        return app?.newEventCalendars() ?? NewEventCalendars(groups: [], named: false)
    }
}
