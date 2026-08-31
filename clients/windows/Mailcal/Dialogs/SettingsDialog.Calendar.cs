// The display pickers, first day of the week, the 12/24-hour clock, and the default horizon,
// the Windows twin of Android's SettingsCalendar.kt. The week start and horizon fill Settings →
// Calendar; the clock is built here beside them (the three are one family) but drawn by
// BuildGeneral, because it spans mail AND calendar and every platform files it under General
// (docs/settings.md). Split into its own partial to keep SettingsDialog.cs under the 500-line
// limit.
//
// All three are persisted in the **core**, not here: three clients disagreeing about which day a week
// starts on silently shifts every column of the grid (docs/calendar.md §3), and the clock must read
// the same in mail and calendar. This file only draws the pickers; the core owns the values, the
// defaults (Monday, 24-hour, 12h horizon) and the clamps. Each pick writes back through the model,
// the core re-signals Settings + Calendar, and the calendar re-applies (CalendarView reacts to
// MailboxModel.DisplaySettingsVersion).
using System.Globalization;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The horizons the picker offers, in hours of the day, the same short list Android shows, between
    // the core's clamp of 4 and 24. The pinch gesture is the fine-grained control; a slider here would
    // just be a worse version of it.
    private static readonly int[] CalendarHorizons = [6, 8, 12, 16, 24];

    private UIElement BuildCalendar()
    {
        if (_model.CalendarDisplaySettings() is not { } display)
        {
            // No core yet (no account connected), nothing to configure.
            return new TextBlock
            {
                Text = L10n.SettingsAccountsEmpty(),
                TextWrapping = TextWrapping.Wrap,
                Opacity = 0.7,
            };
        }

        var panel = new StackPanel { Spacing = 20 };

        // First day of the week, the one setting that shifts every column of the grid if wrong.
        var week = "week-start";
        var weekStack = new StackPanel { Spacing = 4 };
        weekStack.Children.Add(Radio(
            L10n.SettingsWeekStartMonday(), week, display.WeekStart == WeekStart.Monday,
            () => _model.SetWeekStart(WeekStart.Monday)));
        weekStack.Children.Add(Radio(
            L10n.SettingsWeekStartSunday(), week, display.WeekStart == WeekStart.Sunday,
            () => _model.SetWeekStart(WeekStart.Sunday)));
        panel.Children.Add(Group(L10n.SettingsWeekStartHeading(), L10n.SettingsWeekStartDescription(), weekStack));

        // The default horizon, the same number a settled pinch lands on, so the two controls are one
        // setting rather than two that drift apart.
        var horizon = "horizon";
        var horizonStack = new StackPanel { Spacing = 4 };
        foreach (var hours in CalendarHorizons)
        {
            horizonStack.Children.Add(Radio(
                L10n.SettingsHorizonHours(hours.ToString(CultureInfo.InvariantCulture)),
                horizon,
                display.VisibleHours == hours,
                () => _model.SetCalendarVisibleHours(hours)));
        }
        panel.Children.Add(Group(L10n.SettingsHorizonHeading(), L10n.SettingsHorizonDescription(), horizonStack));

        return panel;
    }

    // The 12/24-hour clock group, drawn by BuildGeneral, under General, not Calendar, because it
    // spans mail AND calendar (one app must not disagree with itself) and macOS/Android file it
    // the same way. Null until a core exists (no account connected), in which case General simply
    // omits it.
    private UIElement? TimeFormatGroup()
    {
        if (_model.CalendarDisplaySettings() is not { } display)
        {
            return null;
        }
        var clock = "time-format";
        var clockStack = new StackPanel { Spacing = 4 };
        clockStack.Children.Add(Radio(
            L10n.SettingsTimeFormat24(), clock, display.TimeFormat == TimeFormat.TwentyFourHour,
            () => _model.SetTimeFormat(TimeFormat.TwentyFourHour)));
        clockStack.Children.Add(Radio(
            L10n.SettingsTimeFormat12(), clock, display.TimeFormat == TimeFormat.TwelveHour,
            () => _model.SetTimeFormat(TimeFormat.TwelveHour)));
        return Group(L10n.SettingsTimeFormatHeading(), L10n.SettingsTimeFormatDescription(), clockStack);
    }
}
