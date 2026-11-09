// Every localised string the calendar grid shows, assembled here.
//
// The Rust core emits machine-readable data only, ISO dates, wall-clock minutes, and owns no locale
// facility at all (AGENTS.md: "Localisation is client-side"). So the weekday headings, the hour ruler,
// the period title and the spoken labels are built here, from the core's `days` list and the user's
// locale. Pure functions over plain values, so they can be tested without a view.

import Foundation
import MailcalBindings

/// The calendar the grid does its day arithmetic in: the user's **display zone**, not the device's.
///
/// The core lays out in the display zone, so a client that formatted in the device zone would label
/// the columns with different dates than the blocks were positioned against, which shows up only
/// when the two differ, i.e. exactly when the user is travelling and least able to spot it.
func displayCalendar(zone: String?, locale: Locale = L10n.appLocale) -> Calendar {
    var calendar = Calendar(identifier: .gregorian)
    calendar.locale = locale
    if let zone, let timeZone = TimeZone(identifier: zone) {
        calendar.timeZone = timeZone
    }
    return calendar
}

/// The core hands day columns back as `YYYY-MM-DD`. Parsed in the display zone, so the date the core
/// named is the date this client draws.
///
/// By arithmetic rather than a `DateFormatter`: the grid parses every column it draws on every frame
/// of a scroll, and building a formatter costs more than the rest of that frame's work together.
/// Always Gregorian, because the core's dates are, whatever calendar the caller reckons in.
func parseISODate(_ iso: String, calendar: Calendar) -> Date? {
    let parts = iso.split(separator: "-", omittingEmptySubsequences: false)
    guard
        parts.count == 3, parts[0].count == 4, parts[1].count == 2, parts[2].count == 2,
        let year = Int(parts[0]), let month = Int(parts[1]), let day = Int(parts[2])
    else { return nil }
    let gregorian = isoCalendar(zone: calendar.timeZone)
    guard
        let date = gregorian.date(from: DateComponents(year: year, month: month, day: day)),
        gregorian.dateComponents([.year, .month, .day], from: date)
            == DateComponents(year: year, month: month, day: day)
    else { return nil }
    return date
}

/// A `Date` back to the core's `YYYY-MM-DD`.
func isoDate(_ date: Date, calendar: Calendar) -> String {
    let parts = isoCalendar(zone: calendar.timeZone).dateComponents([.year, .month, .day], from: date)
    return String(format: "%04d-%02d-%02d", parts.year ?? 0, parts.month ?? 0, parts.day ?? 0)
}

private func isoCalendar(zone: TimeZone) -> Calendar {
    var gregorian = Calendar(identifier: .gregorian)
    gregorian.timeZone = zone
    return gregorian
}

/// A formatter for a localised template, built once per template, locale and zone.
///
/// The headings and the title are formatted on every frame of a scroll, and a `DateFormatter` is
/// far more expensive to build than to use. Formatting with one is thread-safe; the lock guards
/// only the dictionary.
private final class TemplateFormatters: @unchecked Sendable {
    private let lock = NSLock()
    private var formatters: [String: DateFormatter] = [:]

    func formatter(_ template: String, calendar: Calendar, locale: Locale) -> DateFormatter {
        let key = "\(template)|\(locale.identifier)|\(calendar.identifier)|\(calendar.timeZone.identifier)"
        lock.lock()
        defer { lock.unlock() }
        if let cached = formatters[key] { return cached }
        let formatter = DateFormatter()
        formatter.calendar = calendar
        formatter.timeZone = calendar.timeZone
        formatter.locale = locale
        formatter.setLocalizedDateFormatFromTemplate(template)
        formatters[key] = formatter
        return formatter
    }
}

private let templateFormatters = TemplateFormatters()

/// The abbreviated weekday for a column heading: "Mon", "ma".
func weekdayShort(_ date: Date, calendar: Calendar, locale: Locale = L10n.appLocale) -> String {
    templateFormatters.formatter("EEE", calendar: calendar, locale: locale).string(from: date)
}

/// The ISO-8601 week number, the "wk 28" a Dutch or German user expects to see.
///
/// Always the ISO reckoning, never the locale's: an ISO week starts on Monday and belongs to the year
/// holding its Thursday. Asking a US-locale calendar for a "week of year" gives a different number,
/// and a week number that disagrees with everyone else's is worse than none.
func isoWeekNumber(_ date: Date, zone: TimeZone) -> Int {
    var iso = Calendar(identifier: .iso8601)
    iso.timeZone = zone
    return iso.component(.weekOfYear, from: date)
}

/// The title over the grid: the month (and year) the shown days fall in.
///
/// A week straddling a month names both, "Jun – Jul 2026", because titling it with one month is
/// wrong for half the columns on screen.
func periodTitle(days: [Date], calendar: Calendar, locale: Locale = L10n.appLocale) -> String {
    guard let first = days.first, let last = days.last else { return "" }
    let month = templateFormatters.formatter("MMM", calendar: calendar, locale: locale)

    let firstYear = calendar.component(.year, from: first)
    let lastYear = calendar.component(.year, from: last)
    let firstMonth = calendar.component(.month, from: first)
    let lastMonth = calendar.component(.month, from: last)

    if firstYear != lastYear {
        return "\(month.string(from: first)) \(firstYear) – \(month.string(from: last)) \(lastYear)"
    }
    if firstMonth != lastMonth {
        return "\(month.string(from: first)) – \(month.string(from: last)) \(lastYear)"
    }
    return "\(month.string(from: first)) \(firstYear)"
}

/// The title over the month grid, the anchored month, not the days on screen.
///
/// A month grid deliberately shows a few days of its neighbours, so titling it from its columns would
/// name June for a July page.
func monthTitle(_ anchor: Date, calendar: Calendar, locale: Locale = L10n.appLocale) -> String {
    templateFormatters.formatter("MMMM yyyy", calendar: calendar, locale: locale).string(from: anchor)
}

/// An hour label for the ruler: "09" on a 24-hour clock, "9 AM" on a 12-hour one.
///
/// Midnight is not labelled, its label would collide with the day heading directly above it.
func hourLabel(_ hour: Int, use24Hour: Bool) -> String {
    guard hour != 0 else { return "" }
    if use24Hour { return String(format: "%02d", hour) }
    if hour < 12 { return "\(hour) AM" }
    if hour == 12 { return "12 PM" }
    return "\(hour - 12) PM"
}

/// Wall-clock minutes from midnight as a clock time: "09:30", or "9:30 AM".
func clockTime(_ minutes: Int, use24Hour: Bool) -> String {
    let hour = min(max(minutes / 60, 0), 23)
    let minute = minutes % 60
    if use24Hour { return String(format: "%02d:%02d", hour, minute) }
    // Two traps in 12-hour: midnight is "12 AM", not "0 AM", and noon is "12 PM", not "0 PM".
    let suffix = hour < 12 ? "AM" : "PM"
    let twelve = hour == 0 ? 12 : (hour > 12 ? hour - 12 : hour)
    return String(format: "%d:%02d %@", twelve, minute, suffix)
}

/// The time a block spans, for its spoken label: "09:30 – 09:45".
func timeRange(_ start: Int, _ end: Int, use24Hour: Bool) -> String {
    "\(clockTime(start, use24Hour: use24Hour)) – \(clockTime(end, use24Hour: use24Hour))"
}
