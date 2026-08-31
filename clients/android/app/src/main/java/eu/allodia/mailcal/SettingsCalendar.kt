// The display settings the calendar (and, for the clock, the mail list) reads: the first day of the
// week, the 12/24-hour clock, the light/dark appearance, and the default horizon.
//
// All of them are persisted in the **core**, not here, three clients disagreeing about which day a
// week starts on silently shifts every column of the grid. This file only draws the pickers; the
// core owns the values, the defaults (Monday, 24-hour) and the clamps.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.CalendarRow
import uniffi.mailcal_bindings.DisplaySettings
import uniffi.mailcal_bindings.TimeFormat
import uniffi.mailcal_bindings.WeekStart

// The horizons the picker offers, in hours of the day. Between the core's clamp of 4 and 24, a
// short list of sensible stops, because the pinch gesture is the fine-grained control and a slider
// here would just be a worse version of it.
private val HORIZON_CHOICES = listOf(6, 8, 12, 16, 24)

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
 * that drift apart.
 */
@Composable
internal fun CalendarHorizonSettingsRows(
    display: DisplaySettings,
    onSetVisibleHours: (Int) -> Unit,
) {
    val ctx = LocalContext.current
    HORIZON_CHOICES.forEach { hours ->
        StrategyRow(
            label = L10n.settings_horizon_hours(ctx, hours.toString()),
            selected = display.visibleHours.toInt() == hours,
        ) { onSetVisibleHours(hours) }
    }
}

/**
 * Which calendar a new event is filed on, and so which colour a slot drawn on the grid wears.
 *
 * Lists only the calendars that can actually take a write: offering a read-only one would produce a
 * default that fails at save time, with the event already typed.
 *
 * The selection comes from the core's own [CalendarRow.isDefault], **not** from a rule repeated
 * here. The core resolves the stored choice against the calendars that exist, falling back when it
 * has been removed or has turned read-only, so this list and the calendar the editor opens on
 * cannot disagree, and neither can four clients.
 */
@Composable
internal fun DefaultCalendarSettingsRows(
    calendars: List<CalendarRow>,
    onSetDefaultCalendar: (account: String?, calendar: String?) -> Unit,
) {
    val ctx = LocalContext.current
    val writable = calendars.filter { it.canWrite }
    if (writable.isEmpty()) {
        Text(
            text = L10n.settings_default_calendar_none(ctx),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        return
    }
    // Grouped by account, the same shape the calendar manager uses, a calendar id is unique only
    // within its account, so the account has to be on screen somewhere. Once per group rather than
    // once per row: an account id is `address@provider-host`, and repeating it beside every calendar
    // wrapped each row over three lines and buried the name the user is actually choosing between.
    //
    // A single account states itself, the rows are its calendars and there is nothing to tell apart
    // so the header only earns its place when there is more than one.
    val byAccount = writable.groupBy { it.account }
    byAccount.forEach { (account, rows) ->
        if (byAccount.size > 1) {
            Text(
                text = account,
                modifier = Modifier.padding(top = 8.dp, bottom = 2.dp),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
            )
        }
        rows.forEach { calendar ->
            StrategyRow(label = calendar.name, selected = calendar.isDefault) {
                onSetDefaultCalendar(calendar.account, calendar.id)
            }
        }
    }
}
