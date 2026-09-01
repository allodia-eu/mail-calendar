// The repeat controls inside the event editor: a frequency, how many periods to skip, which
// weekdays a weekly rule falls on, and what ends it.
//
// Four controls, which is less than a rule can say. The parts they do not model (a monthly series
// pinned to the month's last day, or to a weekday's position in it) ride along in the draft's
// Stored rule and are put back by the core, so an edit that never touched the repeat cannot rewrite
// it. Which rules may be opened at all is the core's answer too: EventDetail.RepeatDraft is absent
// for a rule it could not state in full, and then the summary is shown with no controls.
//
// The panel rebuilds itself whenever the choice changes, because which rows exist depends on it.
// The decisions it draws are EventRepeatChoices (Calendar/), which is WinUI- and L10n-free so
// Mailcal.Tests can reach them; only the words and the controls are here.
using System;
using System.Globalization;
using System.Linq;
using Allodia.Mailcal.Calendar;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

internal static class EventRepeatEditor
{
    /// <summary>The panel holding the controls; it repopulates itself as the choice changes.</summary>
    internal static StackPanel Build(EventEditorState state, Func<string, UIElement> caption)
    {
        var panel = new StackPanel { Spacing = 8 };
        Populate(panel, state, caption);
        return panel;
    }

    private static void Populate(StackPanel panel, EventEditorState state, Func<string, UIElement> caption)
    {
        panel.Children.Clear();
        var draft = state.RepeatDraft;

        var frequency = new ComboBox
        {
            Header = L10n.EventRepeat(),
            ItemsSource = EventRepeatChoices.All.Select(Label).ToList(),
            SelectedIndex = Array.IndexOf(
                EventRepeatChoices.All, EventRepeatChoices.ChoiceOf(draft?.Frequency)),
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        AutomationProperties.SetAutomationId(frequency, "EventRepeatFrequency");
        frequency.SelectionChanged += (_, _) =>
        {
            var picked = EventRepeatChoices.All[Math.Max(0, frequency.SelectedIndex)];
            state.RepeatDraft = picked == RepeatChoice.Never
                ? null
                : state.RepeatDraft is { } held
                    ? held with { Frequency = EventRepeatChoices.Frequency(picked) }
                    : new RepeatDraft(
                        EventRepeatChoices.Frequency(picked),
                        1u,
                        [EventRepeatChoices.WeekdayOf(state.Start)],
                        new RecurrenceEnd.Never(),
                        null);
            Populate(panel, state, caption);
        };
        panel.Children.Add(frequency);

        if (draft is null)
        {
            return;
        }

        panel.Children.Add(IntervalBox(panel, state, caption, draft));

        if (draft.Frequency == RecurrenceFrequency.Weekly)
        {
            panel.Children.Add(WeekdayRow(panel, state, caption, draft));
        }

        var ends = new ComboBox
        {
            Header = L10n.EventRepeatEnds(),
            ItemsSource = EventRepeatChoices.AllEnds.Select(EndLabel).ToList(),
            SelectedIndex = Array.IndexOf(
                EventRepeatChoices.AllEnds, EventRepeatChoices.EndChoiceOf(draft.End)),
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        AutomationProperties.SetAutomationId(ends, "EventRepeatEnds");
        ends.SelectionChanged += (_, _) =>
        {
            if (state.RepeatDraft is not { } held)
            {
                return;
            }
            state.RepeatDraft = held with
            {
                End = EventRepeatChoices.AllEnds[Math.Max(0, ends.SelectedIndex)] switch
                {
                    RepeatEndChoice.Never => new RecurrenceEnd.Never(),
                    // A year out: far enough to be a deliberate choice, near enough to reach.
                    RepeatEndChoice.OnDate => new RecurrenceEnd.OnDate(
                        EventRepeatChoices.EndDateWallClock(state.Start.AddYears(1))),
                    _ => new RecurrenceEnd.AfterCount(10u),
                },
            };
            Populate(panel, state, caption);
        };
        panel.Children.Add(ends);

        switch (draft.End)
        {
            case RecurrenceEnd.OnDate on:
                panel.Children.Add(EndDatePicker(panel, state, caption, on));
                break;
            case RecurrenceEnd.AfterCount after:
                panel.Children.Add(EndCountBox(panel, state, caption, after));
                break;
        }

        if (!string.IsNullOrEmpty(state.Editing?.Occurrence))
        {
            panel.Children.Add(caption(L10n.EventRepeatSeriesNote()));
        }
    }

    private static NumberBox IntervalBox(
        StackPanel panel, EventEditorState state, Func<string, UIElement> caption, RepeatDraft draft)
    {
        var box = Spinner(
            IntervalLabel(EventRepeatChoices.ChoiceOf(draft.Frequency), draft.Interval),
            draft.Interval,
            "EventRepeatInterval");
        box.ValueChanged += (_, _) =>
        {
            var next = Clamped(box.Value);
            if (state.RepeatDraft is { } held && held.Interval != next)
            {
                state.RepeatDraft = held with { Interval = next };
                Populate(panel, state, caption);
            }
        };
        return box;
    }

    private static NumberBox EndCountBox(
        StackPanel panel,
        EventEditorState state,
        Func<string, UIElement> caption,
        RecurrenceEnd.AfterCount after)
    {
        var box = Spinner(
            L10n.EventRepeatEndsTimes((int)after.Count), after.Count, "EventRepeatEndCount");
        box.ValueChanged += (_, _) =>
        {
            var next = Clamped(box.Value);
            if (state.RepeatDraft is { } held && after.Count != next)
            {
                state.RepeatDraft = held with { End = new RecurrenceEnd.AfterCount(next) };
                Populate(panel, state, caption);
            }
        };
        return box;
    }

    private static NumberBox Spinner(string header, uint value, string automationId)
    {
        var box = new NumberBox
        {
            Header = header,
            Value = value,
            Minimum = 1,
            Maximum = EventRepeatChoices.Ceiling,
            SmallChange = 1,
            SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Inline,
        };
        AutomationProperties.SetAutomationId(box, automationId);
        return box;
    }

    /// <summary>A cleared NumberBox reads back NaN, which would otherwise become a zero interval,
    /// a rule the core refuses.</summary>
    private static uint Clamped(double value) =>
        double.IsNaN(value) ? 1u : (uint)Math.Clamp(value, 1, EventRepeatChoices.Ceiling);

    private static StackPanel WeekdayRow(
        StackPanel panel, EventEditorState state, Func<string, UIElement> caption, RepeatDraft draft)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4 };
        var culture = CultureInfo.CurrentCulture;
        var order = EventRepeatChoices.LocalWeekOrder(culture);
        foreach (var day in order)
        {
            var picked = day;
            var toggle = new ToggleButton
            {
                Content = culture.DateTimeFormat.GetShortestDayName(EventRepeatChoices.DayOf(day)),
                IsChecked = draft.Weekdays.Contains(day),
                MinWidth = 40,
            };
            // The button shows an initial; a screen reader gets the whole word.
            AutomationProperties.SetName(
                toggle, culture.DateTimeFormat.GetDayName(EventRepeatChoices.DayOf(day)));
            toggle.Click += (_, _) =>
            {
                if (state.RepeatDraft is { } held)
                {
                    state.RepeatDraft = held with
                    {
                        Weekdays = EventRepeatChoices.Toggled(held.Weekdays, picked, order),
                    };
                }
                Populate(panel, state, caption);
            };
            row.Children.Add(toggle);
        }
        return row;
    }

    private static CalendarDatePicker EndDatePicker(
        StackPanel panel,
        EventEditorState state,
        Func<string, UIElement> caption,
        RecurrenceEnd.OnDate on)
    {
        var picker = new CalendarDatePicker
        {
            Header = L10n.EventRepeatEndsDate(),
            Date = new DateTimeOffset(
                EventRepeatChoices.EndDateOf(on.Date, state.Start), TimeSpan.Zero),
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        AutomationProperties.SetAutomationId(picker, "EventRepeatEndDate");
        picker.DateChanged += (_, _) =>
        {
            if (picker.Date is { } date && state.RepeatDraft is { } held)
            {
                state.RepeatDraft = held with
                {
                    End = new RecurrenceEnd.OnDate(
                        EventRepeatChoices.EndDateWallClock(date.DateTime)),
                };
                Populate(panel, state, caption);
            }
        };
        return picker;
    }

    // --- The words ---------------------------------------------------------------------------

    private static string Label(RepeatChoice choice) => choice switch
    {
        RepeatChoice.Never => L10n.EventRepeatNone(),
        RepeatChoice.Daily => L10n.EventRepeatDaily(),
        RepeatChoice.Weekly => L10n.EventRepeatWeekly(),
        RepeatChoice.Monthly => L10n.EventRepeatMonthly(),
        _ => L10n.EventRepeatYearly(),
    };

    private static string EndLabel(RepeatEndChoice choice) => choice switch
    {
        RepeatEndChoice.Never => L10n.EventRepeatEndsNever(),
        RepeatEndChoice.OnDate => L10n.EventRepeatEndsOnDate(),
        _ => L10n.EventRepeatEndsAfterCount(),
    };

    /// <summary>
    /// "Every 3 weeks": the interval box's own header. Never the frequency word: the picker
    /// directly above already shows it, and a header repeating it reads as a duplicate rather than
    /// as the period it sets.
    /// </summary>
    private static string IntervalLabel(RepeatChoice choice, uint interval)
    {
        var count = (int)interval;
        var many = count > 1;
        return choice switch
        {
            RepeatChoice.Daily => many ? L10n.EventRepeatSumDailyN(count) : L10n.EventRepeatEveryDay(),
            RepeatChoice.Weekly => many ? L10n.EventRepeatEveryWeeks(count) : L10n.EventRepeatEveryWeek(),
            RepeatChoice.Monthly =>
                many ? L10n.EventRepeatEveryMonths(count) : L10n.EventRepeatEveryMonth(),
            RepeatChoice.Yearly => many ? L10n.EventRepeatEveryYears(count) : L10n.EventRepeatEveryYear(),
            _ => Label(choice),
        };
    }
}
