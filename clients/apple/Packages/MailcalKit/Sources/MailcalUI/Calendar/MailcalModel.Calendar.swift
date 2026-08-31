// The calendar grid's model surface.
//
// The grid is a **pull with an argument**, not a pushed snapshot. Every other screen waits for a
// `surfaceChanged` signal and then reads one immutable snapshot slot, but a pager renders three
// pages at once (the one in view and its neighbours), and one slot cannot hold three. Worse,
// `dispatch` is fire-and-forget on a multi-threaded runtime, so two quick swipes would race and the
// grid could settle on *last* week after the user had already swiped to next.
//
// So the client owns the anchor and asks the core for exactly the page it wants. `Surface.calendar`
// is demoted to a cache-invalidation signal: it bumps `calendarVersion`, and the views re-pull
// whatever they are showing. A pull cannot arrive out of order.
//
// What the client does NOT decide: which day the week starts on, and whether times read 14:05 or
// 2:05 PM. Those are persisted core settings, three clients disagreeing about them is not a cosmetic
// bug, it silently shifts every column of the grid.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Whether times render on a 24-hour clock, the user's setting, not the device's, so mail and
    /// calendar cannot disagree with each other.
    var use24Hour: Bool { displaySettings.timeFormat == .twentyFourHour }

    /// Whether the calendar week starts on Monday.
    var weekStartsMonday: Bool { displaySettings.weekStart == .monday }

    /// The calendar the grid does its day arithmetic in: the **display zone**, not the device's.
    var gridCalendar: Calendar { displayCalendar(zone: activeZone) }

    /// The page a grid renders: `columns` consecutive days starting at `from`.
    ///
    /// Consecutive from the anchor and snapped to nothing, that is what lets a zoom widen the day
    /// axis without the grid relocating. Synchronous and cheap: it reads an in-memory cache and never
    /// the store or the network, so it can be called freely while paging.
    func calendarPage(from: Date, columns: Int) -> CalendarPage? {
        app?.calendarRange(from: isoDate(from, calendar: gridCalendar), columns: UInt32(columns))
    }

    /// The month containing `anchor`, laid out. A different query, and a different layout.
    func monthPage(anchor: Date) -> MonthPage? {
        app?.monthPage(anchor: isoDate(anchor, calendar: gridCalendar))
    }

    /// The first day of the week containing `date`, per the user's week-start setting.
    ///
    /// The CORE owns the rule, so three clients cannot disagree about which day a week begins on:
    /// but it is applied only when asked, because aligning on every zoom is exactly the jump the
    /// range query exists to avoid.
    func weekStart(of date: Date) -> Date {
        guard let app else { return date }
        let iso = app.weekStartDate(date: isoDate(date, calendar: gridCalendar))
        return parseISODate(iso, calendar: gridCalendar) ?? date
    }

    /// The colours a user may pick a calendar from. The core owns the palette, so a client cannot
    /// introduce an off-palette colour, Allodia Orange is absent by design: it means "action".
    func calendarPalette() -> [String] { MailcalBindings.calendarPalette() }

    // MARK: - The display settings

    /// The shape the calendar opens in, restored from the core.
    var calendarLayout: CalendarLayout { displaySettings.layout }

    func setWeekStart(_ start: WeekStart) {
        displaySettings = DisplaySettings(
            weekStart: start,
            timeFormat: displaySettings.timeFormat,
            appearance: displaySettings.appearance,
            visibleHours: displaySettings.visibleHours,
            layout: displaySettings.layout
        )
        app?.setWeekStart(start: start)
    }

    func setTimeFormat(_ format: TimeFormat) {
        displaySettings = DisplaySettings(
            weekStart: displaySettings.weekStart,
            timeFormat: format,
            appearance: displaySettings.appearance,
            visibleHours: displaySettings.visibleHours,
            layout: displaySettings.layout
        )
        app?.setTimeFormat(format: format)
    }

    /// Sets the app's light/dark appearance, repainting now and persisting the choice.
    ///
    /// The repaint is explicit because the core signals only `Surface::Settings` for this, it
    /// computes nothing from the appearance, so there is no snapshot for one to ride in on.
    func setAppearance(_ appearance: Appearance) {
        displaySettings = DisplaySettings(
            weekStart: displaySettings.weekStart,
            timeFormat: displaySettings.timeFormat,
            appearance: appearance,
            visibleHours: displaySettings.visibleHours,
            layout: displaySettings.layout
        )
        self.appearance = appearance
        app?.setAppearance(appearance: appearance)
    }

    /// Remembers the shape the calendar is being read in, so it opens that way next time, and opens
    /// the same way on the phone, because the core is what remembers it.
    func setCalendarLayout(_ layout: CalendarLayout) {
        guard layout != displaySettings.layout else { return }
        displaySettings = DisplaySettings(
            weekStart: displaySettings.weekStart,
            timeFormat: displaySettings.timeFormat,
            appearance: displaySettings.appearance,
            visibleHours: displaySettings.visibleHours,
            layout: layout
        )
        app?.setCalendarLayout(layout: layout)
    }

    func setVisibleHours(_ hours: Int) {
        let clamped = UInt8(hours.clamped(to: Int(minVisibleHours)...Int(maxVisibleHours)))
        displaySettings = DisplaySettings(
            weekStart: displaySettings.weekStart,
            timeFormat: displaySettings.timeFormat,
            appearance: displaySettings.appearance,
            visibleHours: clamped,
            layout: displaySettings.layout
        )
        app?.setCalendarVisibleHours(hours: clamped)
    }

    // MARK: - The calendar manager

    /// Shows or hides one calendar's events. Applied at page-read time, so the grid redraws at once:
    /// no sync, no network.
    func setCalendarVisible(_ account: String, _ calendar: String, _ visible: Bool) {
        app?.setCalendarVisible(account: account, calendar: calendar, visible: visible)
    }

    /// Overrides one calendar's colour, or clears the override back to the server's.
    func setCalendarColor(_ account: String, _ calendar: String, _ hex: String?) {
        app?.setCalendarColor(account: account, calendar: calendar, hex: hex)
    }
}
