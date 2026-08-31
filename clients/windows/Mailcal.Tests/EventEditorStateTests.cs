// The event editor's decisions, tested without a dialog: the two payload shapes (a zoned wall-clock
// create, an event-zone edit), the all-day inclusive↔exclusive conversion that bites both ways, the
// fields frozen on edit, validity, and the reminder bucketing. Pure values, the wall clocks are plain
// numbers held apart from any zone, so the assertions hold whatever zone the test machine is in. The
// Windows twin of Apple's EventEditorStateTests and Android's EventEditorTest.
using System;
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class EventEditorStateTests
{
    // A fixed "now", built from components so the wall clock is stable on any machine.
    private static readonly DateTime Now = new(2026, 8, 1, 9, 15, 0, DateTimeKind.Unspecified);

    private static EventDetail Detail(
        bool allDay,
        string timezone,
        string start,
        string end,
        bool isRecurring = false,
        int? reminderMinutes = null,
        EventRecurrence? recurrence = null,
        EventAttendee[]? attendees = null) =>
        new(
            "acct",
            "/cal/e.ics",
            "work",
            "Standup",
            allDay,
            timezone,
            start,
            end,
            "Room 2",
            "bring the roadmap",
            reminderMinutes,
            recurrence,
            null,
            isRecurring,
            true,
            null,
            attendees ?? []);

    [Fact]
    public void Created_timed_event_is_a_wall_clock_in_the_device_zone()
    {
        var editor = EventEditorState.Create(
            new CalendarChoice("acct", "work", "Work"), "Europe/Amsterdam", Now);
        editor.Title = "Lunch";
        editor.Location = "Room 6"; // a create can set a location now (engine PR)
        var args = editor.CreateArgs();
        Assert.Equal("Lunch", args.Title);
        Assert.Equal("2026-08-01T10:00:00", args.Start); // next whole hour after 09:15
        Assert.Equal("2026-08-01T11:00:00", args.End);
        Assert.Equal("Europe/Amsterdam", args.Timezone);
        Assert.Equal("acct", args.Account);
        Assert.Equal("work", args.Calendar);
        Assert.False(args.AllDay);
        Assert.Null(args.Notes);
        Assert.Equal("Room 6", args.Location);
    }

    [Fact]
    public void Created_event_with_no_location_sends_none()
    {
        // Empty stays absent, the core turns a null into no LOCATION line at all.
        var editor = EventEditorState.Create(
            new CalendarChoice("acct", "work", "Work"), "Europe/Amsterdam", Now);
        editor.Title = "Lunch";
        Assert.Null(editor.CreateArgs().Location);
    }

    [Fact]
    public void Created_all_day_event_sends_an_exclusive_end_and_no_zone()
    {
        var editor = EventEditorState.Create(
            new CalendarChoice("acct", "work", "Work"), "Europe/Amsterdam", Now);
        editor.Title = "Holiday";
        editor.AllDay = true; // one day: start and end both fall on 2026-08-01
        var args = editor.CreateArgs();
        Assert.True(args.AllDay);
        Assert.Equal("2026-08-01", args.Start);
        Assert.Equal("2026-08-02", args.End); // exclusive
        Assert.Null(args.Timezone);
    }

    [Fact]
    public void Editing_prefills_the_own_wall_clock_and_updates_it()
    {
        var editor = EventEditorState.Edit(
            Detail(false, "Europe/Amsterdam", "2026-01-05T09:30:00", "2026-01-05T10:00:00"), "Work");
        Assert.True(editor.IsEditing);
        Assert.Equal("Standup", editor.Title);
        Assert.Equal("Room 2", editor.Location);

        editor.Title = "Standup (kort)";
        var args = editor.UpdateArgs(thisOccurrenceOnly: false);
        Assert.Equal("acct", args.Account);
        Assert.Equal("/cal/e.ics", args.Key);
        Assert.Equal("Standup (kort)", args.Title);
        Assert.Equal("2026-01-05T09:30:00", args.Start);
        Assert.Equal("2026-01-05T10:00:00", args.End);
        Assert.Equal("Room 2", args.Location);
        Assert.Null(args.Occurrence); // v1 edits the whole series
    }

    [Fact]
    public void Editing_an_all_day_event_shows_the_inclusive_day_and_saves_the_exclusive_one()
    {
        // The detail's end is exclusive (04-02 for a one-day event on the 1st). The editor must show
        // the 1st and save the 2nd again, an off-by-one here grows a one-day event to two.
        var editor = EventEditorState.Edit(Detail(true, "", "2026-04-01", "2026-04-02"), "Work");
        Assert.True(editor.AllDay);
        var args = editor.UpdateArgs(thisOccurrenceOnly: false);
        Assert.Equal("2026-04-01", args.Start);
        Assert.Equal("2026-04-02", args.End);
    }

    [Fact]
    public void All_day_and_calendar_are_frozen_on_edit_but_free_on_create()
    {
        Assert.True(EventEditorState.Create(null, "Europe/Amsterdam", Now).CanEditForm);
        Assert.False(
            EventEditorState.Edit(
                Detail(false, "Europe/Amsterdam", "2026-01-05T09:30:00", "2026-01-05T10:00:00"), "Work")
                .CanEditForm);
    }

    [Fact]
    public void An_editor_is_invalid_without_a_title_or_a_positive_interval()
    {
        var editor = EventEditorState.Create(
            new CalendarChoice("acct", "work", "Work"), "Europe/Amsterdam", Now);
        Assert.False(editor.IsValid); // blank title
        editor.Title = "X";
        Assert.True(editor.IsValid);
        editor.End = editor.Start;
        Assert.False(editor.IsValid); // end must be after start
    }

    [Fact]
    public void Reminders_bucket_into_the_coarsest_exact_unit()
    {
        Assert.IsType<ReminderBucket.None>(CalendarEventSummary.ReminderBucketOf(null));
        Assert.IsType<ReminderBucket.AtStart>(CalendarEventSummary.ReminderBucketOf(0));
        Assert.Equal(new ReminderBucket.Minutes(15), CalendarEventSummary.ReminderBucketOf(15));
        Assert.Equal(new ReminderBucket.Hours(2), CalendarEventSummary.ReminderBucketOf(120));
        Assert.Equal(new ReminderBucket.Days(1), CalendarEventSummary.ReminderBucketOf(1440));
    }
}
