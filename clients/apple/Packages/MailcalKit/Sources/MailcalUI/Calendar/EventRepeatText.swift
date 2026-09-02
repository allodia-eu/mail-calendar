// The repeat rule as a sentence: "Every 2 weeks on Monday, Friday, until 3 Jun 2027".
//
// Wording only. The core decided which sentence the rule gets, it read the event's start for every
// part the rule leaves out, put the weekdays in week order, and dropped the rules it cannot state
// exactly, so this is a `switch` over a closed set and a catalog lookup. Weekday and month names
// come from the platform's own locale data, the way the grid's headings and the detail's dates do
// (`CalendarFormat`), rather than from the catalog: they are the one part of a localised string we
// do not have to translate ourselves.

import Foundation
import MailcalBindings

/// The repeat summary shown on an event's detail and in its editor.
///
/// `summary` is `nil` for an event with no rule, and for one whose rule the core would not state
/// exactly, those get the bare *Repeats*, because approximating states a series the user does not
/// have and nothing on screen would tell them apart.
func recurrenceText(
    _ summary: RepeatSummary?,
    isRecurring: Bool,
    locale: Locale = L10n.appLocale
) -> String {
    guard let summary else {
        return isRecurring ? L10n.event_repeat_other() : L10n.event_repeat_none()
    }
    let rule = rhythmText(summary.rhythm, locale: locale)
    switch summary.stop {
    case .never:
        return rule
    case .onDate(let date):
        return L10n.event_repeat_sum_until(rule: rule, date: endDateText(date, locale: locale))
    case .afterCount(let count):
        return L10n.event_repeat_sum_times(rule: rule, count: Int(count))
    }
}

/// The rhythm alone, without what ends it.
private func rhythmText(_ rhythm: RepeatRhythm, locale: Locale) -> String {
    switch rhythm {
    case .daily(let interval):
        return interval == 1
            ? L10n.event_repeat_daily()
            : L10n.event_repeat_sum_daily_n(count: Int(interval))

    case .weekly(let interval, let days):
        let named = days.map { weekdayName($0, locale: locale) }.joined(separator: ", ")
        return interval == 1
            ? L10n.event_repeat_sum_weekly(days: named)
            : L10n.event_repeat_sum_weekly_n(count: Int(interval), days: named)

    case .monthlyOnDay(let interval, let day):
        return interval == 1
            ? L10n.event_repeat_sum_monthly_day(day: String(day))
            : L10n.event_repeat_sum_monthly_day_n(count: Int(interval), day: String(day))

    case .monthlyOnLastDay(let interval):
        return interval == 1
            ? L10n.event_repeat_sum_monthly_last()
            : L10n.event_repeat_sum_monthly_last_n(count: Int(interval))

    case .monthlyOnWeekday(let interval, let nth, let day):
        let position = positionText(nth: nth, day: day, locale: locale)
        return interval == 1
            ? L10n.event_repeat_sum_monthly_nth(position: position)
            : L10n.event_repeat_sum_monthly_nth_n(count: Int(interval), position: position)

    case .yearlyOnDate(let interval, let month, let day):
        let named = monthName(month, locale: locale)
        return interval == 1
            ? L10n.event_repeat_sum_yearly(day: String(day), month: named)
            : L10n.event_repeat_sum_yearly_n(count: Int(interval), day: String(day), month: named)

    case .yearlyOnWeekday(let interval, let nth, let day, let month):
        let position = positionText(nth: nth, day: day, locale: locale)
        let named = monthName(month, locale: locale)
        return interval == 1
            ? L10n.event_repeat_sum_yearly_nth(position: position, month: named)
            : L10n.event_repeat_sum_yearly_nth_n(
                count: Int(interval), position: position, month: named
            )
    }
}

/// "on the fourth Monday", "na quarta segunda-feira", the phrase both by-weekday sentences drop
/// into, **carrying its own article**.
///
/// The article belongs here rather than in the frame because in some languages it has to agree with
/// the weekday, and the weekday is not known until this point. Italian inflects for *domenica* and
/// Portuguese for *segunda* through *sexta*; the rest of each language's weekdays take the other
/// form. So each position has two wordings, and **which weekdays take the alternative one is stated
/// in the catalog** (`event_repeat_nth_alt_days`, ISO weekday numbers) rather than as a table of
/// genders in here: it is a fact about a language, and it belongs beside that language's words.
/// A language where the question does not arise leaves the set empty and ships the same wording
/// twice.
private func positionText(nth: Int32, day: RecurrenceWeekday, locale: Locale) -> String {
    let weekday = weekdayName(day, locale: locale)
    let alt = altWeekdays(L10n.event_repeat_nth_alt_days()).contains(isoWeekday(day))
    switch nth {
    case 1:
        return alt ? L10n.event_repeat_nth_first_alt(weekday: weekday)
                   : L10n.event_repeat_nth_first(weekday: weekday)
    case 2:
        return alt ? L10n.event_repeat_nth_second_alt(weekday: weekday)
                   : L10n.event_repeat_nth_second(weekday: weekday)
    case 3:
        return alt ? L10n.event_repeat_nth_third_alt(weekday: weekday)
                   : L10n.event_repeat_nth_third(weekday: weekday)
    case 4:
        return alt ? L10n.event_repeat_nth_fourth_alt(weekday: weekday)
                   : L10n.event_repeat_nth_fourth(weekday: weekday)
    case 5:
        return alt ? L10n.event_repeat_nth_fifth_alt(weekday: weekday)
                   : L10n.event_repeat_nth_fifth(weekday: weekday)
    default:
        return alt ? L10n.event_repeat_nth_last_alt(weekday: weekday)
                   : L10n.event_repeat_nth_last(weekday: weekday)
    }
}

/// The catalog's alternative-form weekdays, as ISO numbers. Empty for a language where the ordinal
/// does not inflect, which is why an unparseable entry is simply dropped: the two wordings are the
/// same string there, so nothing on screen can go wrong.
func altWeekdays(_ catalogEntry: String) -> Set<Int> {
    Set(catalogEntry.split(separator: ",").compactMap { Int($0.trimmingCharacters(in: .whitespaces)) })
}

/// The core's weekday as its ISO number, Monday 1 through Sunday 7, which is what the catalog's
/// alternative-form sets are written in.
func isoWeekday(_ day: RecurrenceWeekday) -> Int {
    switch day {
    case .monday: return 1
    case .tuesday: return 2
    case .wednesday: return 3
    case .thursday: return 4
    case .friday: return 5
    case .saturday: return 6
    case .sunday: return 7
    }
}

/// The weekday's initial for the repeat editor's row: one or two letters, from the platform's
/// own list rather than sliced off its name (several languages do not abbreviate by truncating).
func weekdayInitial(_ day: RecurrenceWeekday, locale: Locale = L10n.appLocale) -> String {
    symbols(locale).veryShortWeekdaySymbols[isoWeekday(day) % 7]
}

/// The weekday's full name, which is what a screen reader gets for a control showing an initial.
func weekdayFullName(_ day: RecurrenceWeekday, locale: Locale = L10n.appLocale) -> String {
    weekdayName(day, locale: locale)
}

/// The weekday's name in `locale`, the platform's word for it, not one of ours.
private func weekdayName(_ day: RecurrenceWeekday, locale: Locale) -> String {
    // `weekdaySymbols` is indexed from Sunday, so an ISO number maps in by taking Sunday's 7 to 0.
    symbols(locale).weekdaySymbols[isoWeekday(day) % 7]
}

/// The month's name in `locale`, from a 1-based month number.
private func monthName(_ month: UInt32, locale: Locale) -> String {
    symbols(locale).monthSymbols[Int(month) - 1]
}

/// A formatter used only for its localised symbol lists. Gregorian explicitly: the symbols are
/// indexed by that calendar's month and weekday numbers, which is what the core sends.
private func symbols(_ locale: Locale) -> DateFormatter {
    let formatter = DateFormatter()
    formatter.calendar = Calendar(identifier: .gregorian)
    formatter.locale = locale
    return formatter
}

/// The last date a repeat may start on, written the way the rest of the app writes a date.
private func endDateText(_ iso: String, locale: Locale) -> String {
    let calendar = displayCalendar(zone: nil, locale: locale)
    guard let date = parseISODate(iso, calendar: calendar) else { return iso }
    return date.formatted(Date.FormatStyle(date: .abbreviated, time: .omitted).locale(locale))
}
