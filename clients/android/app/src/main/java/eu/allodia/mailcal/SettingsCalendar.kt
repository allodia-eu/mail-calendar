// The display settings the calendar (and, for the clock, the mail list) reads: the first day of the
// week, the 12/24-hour clock, the light/dark appearance, and the default horizon.
//
// All of them are persisted in the **core**, not here, three clients disagreeing about which day a
// week starts on silently shifts every column of the grid. This file only draws the pickers; the
// core owns the values, the defaults (Monday, 24-hour), the horizons offered and the clamps.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.DisplaySettings
import uniffi.mailcal_bindings.NewEventCalendars
import uniffi.mailcal_bindings.TimeFormat
import uniffi.mailcal_bindings.WeekStart

/** How times are shown, in mail and calendar alike. */
@Composable
internal fun TimeFormatSettingsRows(
    display: DisplaySettings,
    onSetTimeFormat: (TimeFormat) -> Unit,
) {
    val ctx = LocalContext.current
    StrategyRow(
        label = L10n.settings_time_format_24(ctx),
        selected = display.timeFormat == TimeFormat.TWENTY_FOUR_HOUR,
    ) { onSetTimeFormat(TimeFormat.TWENTY_FOUR_HOUR) }
    StrategyRow(
        label = L10n.settings_time_format_12(ctx),
        selected = display.timeFormat == TimeFormat.TWELVE_HOUR,
    ) { onSetTimeFormat(TimeFormat.TWELVE_HOUR) }
}

/**
 * Whether the app is light, dark, or whatever the device is set to.
 *
 * Beside the other display pickers because it is persisted the same way, in the core, so the
 * clients cannot disagree, even though it is the one of them the core computes nothing from.
 */
@Composable
internal fun AppearanceSettingsRows(
    display: DisplaySettings,
    onSetAppearance: (Appearance) -> Unit,
) {
    val ctx = LocalContext.current
    StrategyRow(
        label = L10n.settings_appearance_system(ctx),
        selected = display.appearance == Appearance.SYSTEM,
    ) { onSetAppearance(Appearance.SYSTEM) }
    StrategyRow(
        label = L10n.settings_appearance_light(ctx),
        selected = display.appearance == Appearance.LIGHT,
    ) { onSetAppearance(Appearance.LIGHT) }
    StrategyRow(
        label = L10n.settings_appearance_dark(ctx),
        selected = display.appearance == Appearance.DARK,
    ) { onSetAppearance(Appearance.DARK) }
}

/** Which day the calendar week begins on. */
@Composable
internal fun WeekStartSettingsRows(
    display: DisplaySettings,
    onSetWeekStart: (WeekStart) -> Unit,
) {
    val ctx = LocalContext.current
    StrategyRow(
        label = L10n.settings_week_start_monday(ctx),
        selected = display.weekStart == WeekStart.MONDAY,
    ) { onSetWeekStart(WeekStart.MONDAY) }
    StrategyRow(
        label = L10n.settings_week_start_sunday(ctx),
        selected = display.weekStart == WeekStart.SUNDAY,
    ) { onSetWeekStart(WeekStart.SUNDAY) }
}

/**
 * How much of the day the grid shows at once.
 *
 * The same number the pinch gesture settles on, so the two controls are one setting rather than two
 * that drift apart. The stops are the core's, and include the horizon a pinch left between two.
 */
@Composable
internal fun CalendarHorizonSettingsRows(
    display: DisplaySettings,
    onSetVisibleHours: (Int) -> Unit,
) {
    val ctx = LocalContext.current
    horizonChoices(display).forEach { hours ->
        StrategyRow(
            label = L10n.settings_horizon_hours(ctx, hours.toString()),
            selected = display.visibleHours.toInt() == hours,
        ) { onSetVisibleHours(hours) }
    }
}

/** The horizons Settings offers, in hours, narrowest first, as the core lists them. */
internal fun horizonChoices(display: DisplaySettings): List<Int> =
    display.visibleHoursChoices.map { it.toUByte().toInt() }

/**
 * Which calendar a new event is filed on, and so which colour a slot drawn on the grid wears.
 *
 * The core's `newEventCalendars`: only calendars that can take a write, an account heading only
 * while it is [NewEventCalendars.named], and the selection its own `CalendarRow.isDefault`, resolved
 * against the calendars that exist, so this list and the calendar the editor opens on cannot
 * disagree.
 */
@Composable
internal fun DefaultCalendarSettingsRows(
    offered: NewEventCalendars,
    onSetDefaultCalendar: (account: String?, calendar: String?) -> Unit,
) {
    val ctx = LocalContext.current
    if (offered.groups.isEmpty()) {
        Text(
            text = L10n.settings_default_calendar_none(ctx),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        return
    }
    offered.groups.forEach { group ->
        if (offered.named) {
            Text(
                text = group.address,
                modifier = Modifier.padding(top = 8.dp, bottom = 2.dp),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
            )
        }
        group.calendars.forEach { calendar ->
            StrategyRow(label = calendar.name, selected = calendar.isDefault) {
                onSetDefaultCalendar(calendar.account, calendar.id)
            }
        }
    }
}
