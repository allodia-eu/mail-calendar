// The display settings the calendar (and, for the clock and the appearance, the whole app) reads.
//
// All of them are persisted in the **core**, not here, three clients disagreeing about which day a
// week starts on silently shifts every column of the grid. These are only the pickers; the core owns
// the values, the defaults (Monday, 24-hour) and the clamps.

import MailcalBindings
import SwiftUI

/// The horizons the picker offers. Between the core's clamp of 4 and 24, a short list of sensible
/// stops, because a pinch is the fine-grained control and a slider here would be a worse version of it.
private let horizonChoices = [6, 8, 12, 16, 24]

/// How times are shown, in **mail and calendar alike**.
struct TimeFormatPicker: View {
    var model: MailboxModel

    var body: some View {
        Picker(
            L10n.settings_time_format_heading(),
            selection: Binding(
                get: { model.displaySettings.timeFormat },
                set: { model.setTimeFormat($0) }
            )
        ) {
            Text(L10n.settings_time_format_24()).tag(TimeFormat.twentyFourHour)
            Text(L10n.settings_time_format_12()).tag(TimeFormat.twelveHour)
        }
        .pickerStyle(.inline)
        .labelsHidden()
    }
}

/// Whether the app is light, dark, or whatever the host is set to.
///
/// Beside the other display pickers because it is persisted the same way, even though it is the one
/// of them the core computes nothing from.
struct AppearancePicker: View {
    var model: MailboxModel

    var body: some View {
        Picker(
            L10n.settings_appearance_heading(),
            selection: Binding(
                get: { model.displaySettings.appearance },
                set: { model.setAppearance($0) }
            )
        ) {
            Text(L10n.settings_appearance_system()).tag(Appearance.system)
            Text(L10n.settings_appearance_light()).tag(Appearance.light)
            Text(L10n.settings_appearance_dark()).tag(Appearance.dark)
        }
        .pickerStyle(.inline)
        .labelsHidden()
    }
}

/// Which day the calendar week begins on.
struct WeekStartPicker: View {
    var model: MailboxModel

    var body: some View {
        Picker(
            L10n.settings_week_start_heading(),
            selection: Binding(
                get: { model.displaySettings.weekStart },
                set: { model.setWeekStart($0) }
            )
        ) {
            Text(L10n.settings_week_start_monday()).tag(WeekStart.monday)
            Text(L10n.settings_week_start_sunday()).tag(WeekStart.sunday)
        }
        .pickerStyle(.inline)
        .labelsHidden()
    }
}

/// How much of the day the grid shows at once, the same number a pinch settles on, so the two
/// controls are one setting rather than two that drift apart.
struct CalendarHorizonPicker: View {
    var model: MailboxModel

    var body: some View {
        Picker(
            L10n.settings_horizon_heading(),
            selection: Binding(
                get: { Int(model.displaySettings.visibleHours) },
                set: { model.setVisibleHours($0) }
            )
        ) {
            ForEach(horizonChoices, id: \.self) { hours in
                Text(L10n.settings_horizon_hours(hours: "\(hours)")).tag(hours)
            }
        }
        .pickerStyle(.inline)
        .labelsHidden()
    }
}
