// The event editor's state and the payloads it produces, a value type, deliberately, so the whole
// of the create/edit logic (validity, the all-day inclusive↔exclusive conversion, which fields are
// frozen on edit, the wall-clock-vs-UTC create form) is testable without a view (mirrors Android's
// EventEditorState).
//
// The load-bearing rule: **times are the event's own wall clock.** On CREATE that is the device's
// zone (so a created event reads back the same clock on edit, see build_event_draft's `timezone`);
// on EDIT it is the event's own zone, which the detail read already gave us. The editor never
// converts between zones, it edits wall-clock numbers (held as `Date`s in the device calendar) and
// states which zone they are in, and the core keeps the event in that zone.

import Foundation
import MailcalBindings

/// A calendar a create can target, or the calendar an edited event sits in.
struct CalendarChoice: Equatable {
    let account: String
    let id: String
    let name: String
}

/// The event an editor is editing (absent when creating).
struct EditTarget {
    let account: String
    let key: String
    /// The event's own zone, empty for a floating/all-day event.
    let zone: String
    let isRecurring: Bool
    let reminderMinutes: Int32?
    let recurrence: EventRecurrence?
    /// The rule as a sentence's parts, decided by the core, see `recurrenceText`.
    let repeatSummary: RepeatSummary?
    /// The occurrence this editor was opened on, as the core resolved it, or empty when it was
    /// opened on the series. Non-empty is what makes Save **ask** which occurrences it meant.
    let occurrence: String
    /// Everyone on the event, organiser first. Read-only, attendees change by iTIP, which is a
    /// separate feature.
    let attendees: [EventAttendee]
}

/// The arguments a create dispatches.
struct CreateArgs: Equatable {
    let title: String
    let start: String
    let end: String
    let account: String?
    let calendar: String?
    let allDay: Bool
    let timezone: String?
    let notes: String?
    let location: String?
}

/// The arguments an edit dispatches.
struct UpdateArgs: Equatable {
    let account: String
    let key: String
    let title: String?
    let start: String?
    let end: String?
    let notes: String?
    let location: String?
    let occurrence: String?
}

struct EventEditorState: Identifiable {
    /// A fresh identity per open, so `.sheet(item:)` re-presents for each create/edit.
    let id = UUID()
    let editing: EditTarget?
    /// The zone the wall clocks are in, the device's on create, the event's own on edit.
    let zone: String
    var title: String
    var allDay: Bool
    /// `start`/`end` hold the wall clock as a device-calendar `Date`, so a `DatePicker` shows the
    /// numbers the user typed and the components read back unchanged.
    var start: Date
    var end: Date
    var location: String
    var notes: String
    var calendar: CalendarChoice?

    var isEditing: Bool { editing != nil }

    /// All-day and the calendar are set at create and frozen on edit (the patcher refuses a form or
    /// calendar change), so the toggle and the picker are enabled only when creating.
    var canEditForm: Bool { editing == nil }

    /// Whether saving has to ask *This event · All events* first, true exactly when this editor
    /// was opened on one occurrence of a series. Mirrors `CalendarDragState.asksAboutTheSeries`,
    /// because it is the same question about the same thing.
    var asksAboutTheSeries: Bool { !(editing?.occurrence.isEmpty ?? true) }

    /// Title present, and the interval non-empty (all-day: end day ≥ start day).
    var isValid: Bool {
        guard !title.trimmingCharacters(in: .whitespaces).isEmpty else { return false }
        let cal = Calendar.current
        if allDay {
            return cal.startOfDay(for: end) >= cal.startOfDay(for: start)
        }
        return end > start
    }

    func createArgs() -> CreateArgs {
        if allDay {
            return CreateArgs(
                title: title.trimmingCharacters(in: .whitespaces),
                start: Self.dateOnly(start),
                // The on-screen end day is inclusive; the engine wants the exclusive next day.
                end: Self.dateOnly(Self.nextDay(end)),
                account: calendar?.account,
                calendar: calendar?.id,
                allDay: true,
                timezone: nil,
                notes: notes.isEmpty ? nil : notes,
                location: location.isEmpty ? nil : location
            )
        }
        return CreateArgs(
            title: title.trimmingCharacters(in: .whitespaces),
            start: Self.wallClock(start),
            end: Self.wallClock(end),
            account: calendar?.account,
            calendar: calendar?.id,
            allDay: false,
            // A wall clock in the device's zone, so the event is created there, not in UTC.
            timezone: zone.isEmpty ? nil : zone,
            notes: notes.isEmpty ? nil : notes,
            location: location.isEmpty ? nil : location
        )
    }

    /// The payload a Save dispatches.
    ///
    /// `thisOccurrenceOnly` splits an override out of the series instead of rewriting it. Both
    /// edges always travel: an occurrence's own times are not the series', so a single-occurrence
    /// edit that named neither would move it onto the master's clock.
    func updateArgs(thisOccurrenceOnly: Bool) -> UpdateArgs {
        let target = editing!
        let startStr = allDay ? Self.dateOnly(start) : Self.wallClock(start)
        let endStr = allDay ? Self.dateOnly(Self.nextDay(end)) : Self.wallClock(end)
        return UpdateArgs(
            account: target.account,
            key: target.key,
            title: title.trimmingCharacters(in: .whitespaces),
            start: startStr,
            end: endStr,
            // Empty clears; a value sets.
            notes: notes,
            location: location,
            occurrence: thisOccurrenceOnly && !target.occurrence.isEmpty ? target.occurrence : nil
        )
    }

    // MARK: - Factories

    /// A fresh editor: start at the next whole hour, one hour long, in the default calendar.
    /// From the "New event" button, `now` is the clock and the editor opens at the next whole hour
    /// for an hour, the sensible default when the user has said nothing about *when*. From a
    /// **drag on the grid** they have said exactly when, so `exact` is set: `now` is the start
    /// verbatim and `minutes` the length, and nothing is rounded on top of a time drawn by hand.
    static func create(
        default defaultCalendar: CalendarChoice?,
        zone: String,
        now: Date,
        minutes: Int = 60,
        exact: Bool = false
    ) -> EventEditorState {
        let cal = Calendar.current
        let inOneHour = cal.date(byAdding: .hour, value: 1, to: now) ?? now
        // Set the time *on the same day*, `date(bySetting:)` would search forward to the *next*
        // minute-zero, which from 10:15 is 11:00, not 10:00.
        let rounded = cal.date(
            bySettingHour: cal.component(.hour, from: inOneHour), minute: 0, second: 0, of: inOneHour
        ) ?? inOneHour
        let start = exact ? now : rounded
        return EventEditorState(
            editing: nil,
            zone: zone,
            title: "",
            allDay: false,
            start: start,
            end: cal.date(byAdding: .minute, value: minutes, to: start) ?? start,
            location: "",
            notes: "",
            calendar: defaultCalendar
        )
    }

    /// An editor prefilled from a stored event's detail.
    static func edit(_ detail: EventDetail, calendarName: String) -> EventEditorState {
        let start = Self.parseWall(detail.start)
        // The detail's all-day end is exclusive; show the inclusive last day.
        let end = detail.allDay ? Self.previousDay(Self.parseWall(detail.end)) : Self.parseWall(detail.end)
        return EventEditorState(
            editing: EditTarget(
                account: detail.account,
                key: detail.key,
                zone: detail.timezone,
                isRecurring: detail.isRecurring,
                reminderMinutes: detail.reminderMinutes,
                recurrence: detail.recurrence,
                repeatSummary: detail.repeatSummary,
                occurrence: detail.occurrenceStart,
                attendees: detail.attendees
            ),
            zone: detail.timezone,
            title: detail.title,
            allDay: detail.allDay,
            start: start,
            end: end,
            location: detail.location ?? "",
            notes: detail.notes ?? "",
            calendar: CalendarChoice(account: detail.account, id: detail.calendar, name: calendarName)
        )
    }

    // MARK: - Wall-clock <-> string, in the device calendar (numbers only; the zone is tracked apart)

    private static let wallFields: Set<Calendar.Component> = [.year, .month, .day, .hour, .minute, .second]

    static func wallClock(_ date: Date) -> String {
        let c = Calendar.current.dateComponents(wallFields, from: date)
        return String(
            format: "%04d-%02d-%02dT%02d:%02d:%02d",
            c.year ?? 0, c.month ?? 0, c.day ?? 0, c.hour ?? 0, c.minute ?? 0, c.second ?? 0
        )
    }

    static func dateOnly(_ date: Date) -> String {
        let c = Calendar.current.dateComponents([.year, .month, .day], from: date)
        return String(format: "%04d-%02d-%02d", c.year ?? 0, c.month ?? 0, c.day ?? 0)
    }

    static func nextDay(_ date: Date) -> Date {
        Calendar.current.date(byAdding: .day, value: 1, to: date) ?? date
    }

    static func previousDay(_ date: Date) -> Date {
        Calendar.current.date(byAdding: .day, value: -1, to: date) ?? date
    }

    /// Parse `YYYY-MM-DDTHH:MM:SS` or a bare `YYYY-MM-DD` into a device-calendar `Date` carrying the
    /// same wall-clock numbers.
    static func parseWall(_ value: String) -> Date {
        let parts = value.split(separator: "T")
        let date = parts.first.map(String.init) ?? value
        let dmy = date.split(separator: "-").compactMap { Int($0) }
        var comps = DateComponents()
        if dmy.count == 3 {
            comps.year = dmy[0]
            comps.month = dmy[1]
            comps.day = dmy[2]
        }
        if parts.count > 1 {
            let hms = parts[1].split(separator: ":").compactMap { Int($0) }
            comps.hour = hms.count > 0 ? hms[0] : 0
            comps.minute = hms.count > 1 ? hms[1] : 0
            comps.second = hms.count > 2 ? hms[2] : 0
        }
        return Calendar.current.date(from: comps) ?? Date()
    }
}

/// A reminder offset, bucketed for display, pure, so a locale quirk can't reach it.
enum ReminderBucket: Equatable {
    case none
    case atStart
    case minutes(Int)
    case hours(Int)
    case days(Int)
}

/// Buckets minutes-before into the coarsest exact unit (a day, an hour, else minutes).
func reminderBucket(_ minutes: Int32?) -> ReminderBucket {
    guard let m = minutes.map(Int.init) else { return .none }
    if m <= 0 { return .atStart }
    if m % 1440 == 0 { return .days(m / 1440) }
    if m % 60 == 0 { return .hours(m / 60) }
    return .minutes(m)
}
