// What the repeat controls send, and (more to the point) what they refuse to send.
//
// The rebuild itself is the core's and is tested there; it is stubbed here, because nothing in this
// assembly loads the cdylib. What is this client's, and is tested here, is which of the three
// answers a save carries, plus the two silent failures in the weekday row: DayOfWeek counts Sunday
// as 0 while the core counts from Monday, and a weekly rule left with no day ticked is one the core
// refuses.
using System;
using System.Globalization;
using System.Linq;
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class EventRepeatEditorTests
{
    private static readonly SimpleRecurrence WeeklyRule = new(
        RecurrenceFrequency.Weekly, 1u, [], [], [], new RecurrenceEnd.Never());

    private static readonly RepeatDraft WeeklyDraft = new(
        RecurrenceFrequency.Weekly,
        1u,
        [RecurrenceWeekday.Wednesday],
        new RecurrenceEnd.Never(),
        WeeklyRule);

    /// <summary>
    /// The core's decision, stated the way the core states it, but without the cdylib: a draft
    /// equal to the rule it was seeded from is not a change, and anything else is a Set.
    /// </summary>
    private static RecurrenceChange? StubChangeOf(RepeatDraft? draft, bool wasRepeating)
    {
        if (draft is null)
        {
            return wasRepeating ? new RecurrenceChange.Clear() : null;
        }
        var rebuilt = new SimpleRecurrence(
            draft.Frequency, draft.Interval, [], [], [], draft.End);
        return draft.Stored is { } stored && stored == rebuilt
            ? null
            : new RecurrenceChange.Set(rebuilt);
    }

    private static EventDetail Detail(
        bool isRecurring,
        EventRecurrence? recurrence,
        RepeatDraft? repeatDraft,
        string occurrence = "") => new(
            "acct", "/cal/e.ics", "work", "Standup", false, "Europe/Amsterdam",
            "2026-08-26T09:00:00", "2026-08-26T09:30:00", null, null, null,
            recurrence, null, repeatDraft, isRecurring, true, occurrence, []);

    private static EventEditorState EditorOn(
        bool isRecurring = true,
        EventRecurrence? recurrence = null,
        RepeatDraft? repeatDraft = null,
        string occurrence = "") =>
        EventEditorState.Edit(
            Detail(
                isRecurring,
                recurrence ?? new EventRecurrence.Simple(WeeklyRule),
                repeatDraft ?? WeeklyDraft,
                occurrence),
            "Work",
            StubChangeOf);

    [Fact]
    public void A_save_that_never_touched_the_repeat_says_nothing_about_it() =>
        Assert.Null(EditorOn().UpdateArgs(thisOccurrenceOnly: false).Recurrence);

    [Fact]
    public void A_changed_repeat_is_sent_as_a_set()
    {
        var editor = EditorOn();
        editor.RepeatDraft = editor.RepeatDraft! with { Interval = 2u };

        var change = Assert.IsType<RecurrenceChange.Set>(
            editor.UpdateArgs(thisOccurrenceOnly: false).Recurrence);
        Assert.Equal(2u, change.Rule.Interval);
    }

    [Fact]
    public void Choosing_does_not_repeat_clears_the_series()
    {
        var editor = EditorOn();
        editor.RepeatDraft = null;
        Assert.IsType<RecurrenceChange.Clear>(
            editor.UpdateArgs(thisOccurrenceOnly: false).Recurrence);
    }

    /// <summary>A rule belongs to the series. The core refuses the pairing; the editor never
    /// builds it.</summary>
    [Fact]
    public void A_rule_never_travels_with_a_single_occurrence()
    {
        var editor = EditorOn(occurrence: "2026-09-02T09:00:00");
        editor.RepeatDraft = editor.RepeatDraft! with { Interval = 3u };

        var args = editor.UpdateArgs(thisOccurrenceOnly: true);
        Assert.Equal("2026-09-02T09:00:00", args.Occurrence);
        Assert.Null(args.Recurrence);
    }

    /// <summary>Opened on one occurrence, a save normally asks which occurrences it meant. A
    /// changed rule answers that question on its own, so it is not put.</summary>
    [Fact]
    public void A_changed_repeat_settles_the_scope_question()
    {
        var editor = EditorOn(occurrence: "2026-09-02T09:00:00");
        Assert.True(editor.AsksAboutTheSeries);

        editor.RepeatDraft = editor.RepeatDraft! with { Interval = 2u };
        Assert.False(editor.AsksAboutTheSeries);
    }

    /// <summary>A rule the core would not state is shown and not offered: the client never seeds
    /// an editor from a partial picture, because saving it back would drop the rest.</summary>
    [Fact]
    public void A_rule_too_rich_to_state_offers_no_controls()
    {
        var editor = EventEditorState.Edit(
            Detail(true, new EventRecurrence.Complex(), null), "Work", StubChangeOf);
        Assert.False(editor.CanEditRepeat);
        Assert.Null(editor.UpdateArgs(thisOccurrenceOnly: false).Recurrence);
    }

    [Fact]
    public void An_event_that_does_not_repeat_can_be_given_a_rule()
    {
        var editor = EventEditorState.Edit(Detail(false, null, null), "Work", StubChangeOf);
        Assert.True(editor.CanEditRepeat);
        Assert.Null(editor.UpdateArgs(thisOccurrenceOnly: false).Recurrence);

        editor.RepeatDraft = new RepeatDraft(
            RecurrenceFrequency.Daily,
            1u,
            [RecurrenceWeekday.Wednesday],
            new RecurrenceEnd.Never(),
            null);
        var change = Assert.IsType<RecurrenceChange.Set>(
            editor.UpdateArgs(thisOccurrenceOnly: false).Recurrence);
        Assert.Equal(RecurrenceFrequency.Daily, change.Rule.Frequency);
    }

    [Fact]
    public void A_create_carries_the_rule_as_a_plain_rule_rather_than_an_answer()
    {
        var editor = EventEditorState.Create(
            new CalendarChoice("acct", "work", "Work"),
            "Europe/Amsterdam",
            new DateTime(2026, 8, 26, 9, 0, 0, DateTimeKind.Unspecified),
            StubChangeOf);
        editor.Title = "Standup";
        Assert.Null(editor.CreateArgs().Recurrence);

        editor.RepeatDraft = new RepeatDraft(
            RecurrenceFrequency.Weekly,
            2u,
            [RecurrenceWeekday.Wednesday],
            new RecurrenceEnd.AfterCount(8u),
            null);
        var rule = editor.CreateArgs().Recurrence;
        Assert.Equal(RecurrenceFrequency.Weekly, rule!.Frequency);
        Assert.Equal(2u, rule.Interval);
        Assert.Equal(new RecurrenceEnd.AfterCount(8u), rule.End);
    }

    // --- The pure control logic ---------------------------------------------------------

    /// <summary>A weekly rule that names no day is not a rule, so the last day ticked stays.</summary>
    [Fact]
    public void The_weekday_row_never_empties()
    {
        var order = EventRepeatChoices.LocalWeekOrder(new CultureInfo("en-GB"));
        RecurrenceWeekday[] one = [RecurrenceWeekday.Wednesday];
        Assert.Equal(one, EventRepeatChoices.Toggled(one, RecurrenceWeekday.Wednesday, order));
    }

    [Fact]
    public void Ticking_a_weekday_returns_the_row_in_week_order()
    {
        var order = EventRepeatChoices.LocalWeekOrder(new CultureInfo("en-GB"));
        var ticked = EventRepeatChoices.Toggled(
            [RecurrenceWeekday.Friday], RecurrenceWeekday.Monday, order);
        Assert.Equal([RecurrenceWeekday.Monday, RecurrenceWeekday.Friday], ticked);
    }

    [Fact]
    public void The_week_starts_where_the_culture_starts_it()
    {
        Assert.Equal(
            RecurrenceWeekday.Monday,
            EventRepeatChoices.LocalWeekOrder(new CultureInfo("en-GB"))[0]);
        Assert.Equal(
            RecurrenceWeekday.Sunday,
            EventRepeatChoices.LocalWeekOrder(new CultureInfo("en-US"))[0]);
        Assert.Equal(7, EventRepeatChoices.LocalWeekOrder(new CultureInfo("en-US")).Distinct().Count());
    }

    /// <summary>DayOfWeek counts Sunday as 0 and the core counts from Monday: an off-by-one here
    /// renames every day of the week and still draws a plausible row.</summary>
    [Fact]
    public void A_rule_first_chosen_falls_on_the_events_own_weekday()
    {
        // 26 August 2026 is a Wednesday; 30 August is a Sunday.
        Assert.Equal(RecurrenceWeekday.Wednesday, EventRepeatChoices.WeekdayOf(new DateTime(2026, 8, 26)));
        Assert.Equal(RecurrenceWeekday.Sunday, EventRepeatChoices.WeekdayOf(new DateTime(2026, 8, 30)));
    }

    [Fact]
    public void An_end_date_round_trips_through_the_wall_clock_it_is_written_as()
    {
        var date = new DateTime(2027, 3, 1);
        var written = EventRepeatChoices.EndDateWallClock(date);
        Assert.Equal("2027-03-01T00:00:00", written);
        Assert.Equal(date, EventRepeatChoices.EndDateOf(written, DateTime.MinValue));
    }
}
