// The repeat controls inside the event editor: a frequency, how many periods to skip, which
// weekdays a weekly rule falls on, and what ends it.
//
// Four controls, which is less than a rule can say. The parts they do not model (a monthly series
// pinned to the month's last day, or to a weekday's position in it) ride along in the draft's
// `stored` rule and are put back by the core, so an edit that never touched the repeat cannot
// rewrite it. Which rules may be opened at all is the core's answer too: `EventDetail.repeatDraft`
// is absent for a rule it could not state in full, and then the summary is shown with no controls.

import MailcalBindings
import SwiftUI

/// What the frequency picker offers, including the choice not to repeat.
enum RepeatChoice: Hashable, CaseIterable {
    case never, daily, weekly, monthly, yearly

    init(_ frequency: RecurrenceFrequency?) {
        switch frequency {
        case .none: self = .never
        case .daily: self = .daily
        case .weekly: self = .weekly
        case .monthly: self = .monthly
        case .yearly: self = .yearly
        }
    }

    var frequency: RecurrenceFrequency? {
        switch self {
        case .never: return nil
        case .daily: return .daily
        case .weekly: return .weekly
        case .monthly: return .monthly
        case .yearly: return .yearly
        }
    }

    var label: String {
        switch self {
        case .never: return L10n.event_repeat_none()
        case .daily: return L10n.event_repeat_daily()
        case .weekly: return L10n.event_repeat_weekly()
        case .monthly: return L10n.event_repeat_monthly()
        case .yearly: return L10n.event_repeat_yearly()
        }
    }

    /// "Every 3 weeks": the interval stepper's own label, which reads as a sentence rather than
    /// as a number beside a unit nobody set.
    func intervalLabel(_ interval: UInt32) -> String {
        let count = Int(interval)
        // Never the frequency word: the picker directly above already shows it, and a stepper
        // repeating it reads as a duplicate rather than as the period it sets.
        switch (self, count > 1) {
        case (.never, _): return label
        case (.daily, false): return L10n.event_repeat_every_day()
        case (.daily, true): return L10n.event_repeat_sum_daily_n(count: count)
        case (.weekly, false): return L10n.event_repeat_every_week()
        case (.weekly, true): return L10n.event_repeat_every_weeks(count: count)
        case (.monthly, false): return L10n.event_repeat_every_month()
        case (.monthly, true): return L10n.event_repeat_every_months(count: count)
        case (.yearly, false): return L10n.event_repeat_every_year()
        case (.yearly, true): return L10n.event_repeat_every_years(count: count)
        }
    }
}

/// What the "Ends" picker offers.
enum RepeatEndChoice: Hashable, CaseIterable {
    case never, onDate, afterCount

    init(_ end: RecurrenceEnd) {
        switch end {
        case .never: self = .never
        case .onDate: self = .onDate
        case .afterCount: self = .afterCount
        }
    }

    var label: String {
        switch self {
        case .never: return L10n.event_repeat_ends_never()
        case .onDate: return L10n.event_repeat_ends_on_date()
        case .afterCount: return L10n.event_repeat_ends_after_count()
        }
    }
}

/// The most periods, and the most instances, either stepper will go to. Well under the core's own
/// ceiling, which refuses a rule no calendar could draw.
private let repeatCeiling = 999

/// The weekdays in the order this device's locale starts its week on, so the row reads the way
/// every other calendar on the machine does.
var localWeekOrder: [RecurrenceWeekday] {
    let week: [RecurrenceWeekday] = [
        .sunday, .monday, .tuesday, .wednesday, .thursday, .friday, .saturday,
    ]
    let first = max(1, min(7, Calendar.current.firstWeekday)) - 1
    return Array(week[first...] + week[..<first])
}

/// The weekday a rule first chosen on this event should fall on.
func recurrenceWeekday(of date: Date, calendar: Calendar = .current) -> RecurrenceWeekday {
    // `.weekday` counts Sunday as 1, which is the order `week` above is written in.
    let week: [RecurrenceWeekday] = [
        .sunday, .monday, .tuesday, .wednesday, .thursday, .friday, .saturday,
    ]
    let index = calendar.component(.weekday, from: date) - 1
    return week[max(0, min(6, index))]
}

struct EventRepeatSection: View {
    @Binding var draft: RepeatDraft?
    /// The event's start, where a rule chosen for the first time takes its weekday from.
    let start: Date
    /// Whether this editor was opened on one occurrence, in which case changing the repeat is
    /// stated to reach the whole series before the user touches a control.
    let opensOnOneOccurrence: Bool

    private var choice: RepeatChoice { RepeatChoice(draft?.frequency) }

    var body: some View {
        Picker(L10n.event_repeat(), selection: choiceBinding) {
            ForEach(RepeatChoice.allCases, id: \.self) { option in
                Text(option.label).tag(option)
            }
        }

        if let draft {
            Stepper(
                choice.intervalLabel(draft.interval),
                value: intervalBinding,
                in: 1...repeatCeiling
            )

            if draft.frequency == .weekly {
                weekdayRow
            }

            Picker(L10n.event_repeat_ends(), selection: endBinding) {
                ForEach(RepeatEndChoice.allCases, id: \.self) { option in
                    Text(option.label).tag(option)
                }
            }

            switch draft.end {
            case .onDate:
                DatePicker(
                    L10n.event_repeat_ends_date(),
                    selection: endDateBinding,
                    displayedComponents: [.date]
                )
            case .afterCount(let count):
                Stepper(
                    L10n.event_repeat_ends_times(count: Int(count)),
                    value: endCountBinding,
                    in: 1...repeatCeiling
                )
            case .never:
                EmptyView()
            }
        }

        if draft != nil, opensOnOneOccurrence {
            Text(L10n.event_repeat_series_note())
                .font(.footnote)
                .foregroundStyle(.secondary)
        }
    }

    /// The weekdays a weekly rule falls on. At least one stays ticked: a weekly rule that names
    /// no day is not a rule, and the core would refuse it.
    private var weekdayRow: some View {
        HStack(spacing: 6) {
            ForEach(localWeekOrder, id: \.self) { day in
                let on = draft?.weekdays.contains(day) ?? false
                Button {
                    toggle(day)
                } label: {
                    Text(weekdayInitial(day))
                        .font(.footnote.weight(on ? .semibold : .regular))
                        .frame(maxWidth: .infinity, minHeight: 30)
                        .background(on ? Color.accentColor : Color.clear, in: Capsule())
                        .foregroundStyle(on ? Color.white : Color.primary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(weekdayFullName(day))
                .accessibilityAddTraits(on ? [.isSelected] : [])
            }
        }
    }

    private func toggle(_ day: RecurrenceWeekday) {
        guard var current = draft else { return }
        var days = current.weekdays
        if let at = days.firstIndex(of: day) {
            // Never leave a weekly rule with no day: the last one ticked stays ticked.
            guard days.count > 1 else { return }
            days.remove(at: at)
        } else {
            days.append(day)
        }
        current.weekdays = localWeekOrder.filter { days.contains($0) }
        draft = current
    }

    // MARK: - Bindings

    private var choiceBinding: Binding<RepeatChoice> {
        Binding(
            get: { choice },
            set: { picked in
                guard let frequency = picked.frequency else {
                    draft = nil
                    return
                }
                guard var current = draft else {
                    draft = RepeatDraft(
                        frequency: frequency,
                        interval: 1,
                        weekdays: [recurrenceWeekday(of: start)],
                        end: .never,
                        stored: nil
                    )
                    return
                }
                current.frequency = frequency
                draft = current
            }
        )
    }

    private var intervalBinding: Binding<Int> {
        Binding(
            get: { Int(draft?.interval ?? 1) },
            set: { draft?.interval = UInt32(max(1, $0)) }
        )
    }

    private var endBinding: Binding<RepeatEndChoice> {
        Binding(
            get: { RepeatEndChoice(draft?.end ?? .never) },
            set: { picked in
                switch picked {
                case .never:
                    draft?.end = .never
                case .onDate:
                    // A year out: far enough to be a deliberate choice, near enough to reach.
                    let year = Calendar.current.date(byAdding: .year, value: 1, to: start) ?? start
                    draft?.end = .onDate(date: EventEditorState.wallClock(year))
                case .afterCount:
                    draft?.end = .afterCount(count: 10)
                }
            }
        )
    }

    private var endDateBinding: Binding<Date> {
        Binding(
            get: {
                guard case .onDate(let date) = draft?.end else { return start }
                return EventEditorState.parseWall(date)
            },
            set: { draft?.end = .onDate(date: EventEditorState.wallClock($0)) }
        )
    }

    private var endCountBinding: Binding<Int> {
        Binding(
            get: {
                guard case .afterCount(let count) = draft?.end else { return 1 }
                return Int(count)
            },
            set: { draft?.end = .afterCount(count: UInt32(max(1, $0))) }
        )
    }
}
