// The event editor's state and the payloads it produces, a plain class, deliberately, so the whole
// of the create/edit logic (validity, the all-day inclusive↔exclusive conversion, which fields are
// frozen on edit, the wall-clock-vs-UTC create form) is testable in the plain net10.0 test assembly
// without constructing a ContentDialog (AGENTS.md). The WinUI twin of Android's EventEditorState.kt
// and Apple's EventEditorState.swift, and it deliberately produces the SAME two payload shapes.
//
// The one rule that is load-bearing and easy to get wrong: **times are the event's own wall clock.**
// On CREATE that is the device's zone (so a created event reads back the same clock on edit, see
// build_event_draft's `timezone`). On EDIT it is the event's own zone, which the detail read already
// gave us. The editor never converts between zones; it edits a wall clock (held as a `DateTime` with
// Kind=Unspecified, numbers, no zone) and states which zone it is in, and the core keeps it there.
//
// Nothing here touches WinUI: the pickers live in the dialog (Dialogs/EventEditorDialog.cs), which
// reads and writes these plain fields. That seam is what keeps this file in Mailcal.Tests.
using System;
using System.Collections.Generic;
using System.Globalization;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>A calendar a create can target, or the calendar an edited event sits in.</summary>
internal sealed record CalendarChoice(string Account, string Id, string Name);

/// <summary>The event an editor is editing (absent when creating).</summary>
internal sealed record EditTarget(
    string Account,
    string Key,
    /// <summary>The event's own zone, empty for a floating or all-day event.</summary>
    string Zone,
    bool IsRecurring,
    int? ReminderMinutes,
    EventRecurrence? Recurrence,
    /// <summary>The rule as a sentence's parts, decided by the core, see EventRepeatText.</summary>
    RepeatSummary? RepeatSummary,
    /// <summary>The rule as the editor's controls hold it, or null when the core would not open
    /// it: a rule too rich to state, or one whose controls this app does not have.</summary>
    RepeatDraft? SeededRepeat,
    /// <summary>The occurrence this editor was opened on, as the core resolved it, or empty when
    /// it was opened on the series. Non-empty is what makes Save ask which occurrences it
    /// meant.</summary>
    string Occurrence,
    /// <summary>Everyone on the event, organiser first. Shown read-only, attendees change by
    /// iTIP, which is a separate feature.</summary>
    IReadOnlyList<EventAttendee> Attendees);

/// <summary>The arguments a create dispatches (<c>Intent.CreateEvent</c>).</summary>
internal sealed record CreateArgs(
    string Title,
    string Start,
    string End,
    string? Account,
    string? Calendar,
    bool AllDay,
    string? Timezone,
    string? Notes,
    string? Location,
    /// <summary>The rule a new event starts with, or null for a one-off.</summary>
    SimpleRecurrence? Recurrence);

/// <summary>The arguments an edit dispatches (<c>Intent.UpdateEvent</c>).</summary>
internal sealed record UpdateArgs(
    string Account,
    string Key,
    string? Title,
    string? Start,
    string? End,
    string? Notes,
    string? Location,
    string? Occurrence,
    /// <summary>What happens to the repeat rule: null leaves the series alone, <c>Set</c> replaces
    /// the rule, <c>Clear</c> makes the event a single one. Never sent beside an
    /// <see cref="Occurrence"/>: a rule belongs to the series, and the core refuses the
    /// pairing.</summary>
    RecurrenceChange? Recurrence);

/// <summary>
/// What a save should send for the repeat rule, decided by the core: see
/// <c>mailcal_account::recurrence_change_of</c>.
/// </summary>
/// <remarks>
/// Taken as a delegate rather than called directly so <see cref="EventEditorState"/> stays a plain
/// class <c>Mailcal.Tests</c> can drive, and a test can state the client's own rules without the
/// native library the real one needs.
/// </remarks>
internal delegate RecurrenceChange? RepeatChangeOf(RepeatDraft? draft, bool wasRepeating);

/// <summary>
/// The mutable state of an open editor. Construct via <see cref="Create"/> or <see cref="Edit"/>; the
/// dialog binds its fields directly.
/// </summary>
/// <remarks>
/// All the decisions, validity, the frozen-on-edit fields, the two payload shapes, are members
/// here, so a test drives them without a dialog. <see cref="Start"/>/<see cref="End"/> hold the wall
/// clock as a <see cref="DateTime"/> with <see cref="DateTimeKind.Unspecified"/>: plain numbers, in
/// the zone named by <see cref="Zone"/>, never converted.
/// </remarks>
internal sealed class EventEditorState
{
    private EventEditorState(
        EditTarget? editing,
        string zone,
        string title,
        bool allDay,
        DateTime start,
        DateTime end,
        string location,
        string notes,
        CalendarChoice? calendar,
        RepeatDraft? repeat,
        RepeatChangeOf repeatChangeOf)
    {
        _repeatChangeOf = repeatChangeOf;
        RepeatDraft = repeat;
        Editing = editing;
        Zone = zone;
        Title = title;
        AllDay = allDay;
        Start = start;
        End = end;
        Location = location;
        Notes = notes;
        Calendar = calendar;
    }

    private readonly RepeatChangeOf _repeatChangeOf;

    /// <summary>
    /// What the repeat controls hold, or null for "does not repeat". Seeded from the core and
    /// passed back to it. The <c>Stored</c> rule it carries is what tells a rule that changed from
    /// one that did not, and what keeps the parts no control here models.
    /// </summary>
    internal RepeatDraft? RepeatDraft { get; set; }

    /// <summary>The event being edited, or <c>null</c> when creating.</summary>
    internal EditTarget? Editing { get; }

    /// <summary>The zone the wall clocks are in, the device's on create, the event's own on edit.</summary>
    internal string Zone { get; }

    internal string Title { get; set; }

    internal bool AllDay { get; set; }

    /// <summary>The start wall clock (Kind=Unspecified). All-day uses only its date part.</summary>
    internal DateTime Start { get; set; }

    /// <summary>The end wall clock. On screen the all-day end is the <b>inclusive</b> last day.</summary>
    internal DateTime End { get; set; }

    internal string Location { get; set; }

    internal string Notes { get; set; }

    internal CalendarChoice? Calendar { get; set; }

    internal bool IsEditing => Editing is not null;

    /// <summary>
    /// All-day and the calendar are set at create and frozen on edit (the patcher refuses a form or a
    /// calendar change), so the toggle and the picker are enabled only when creating.
    /// </summary>
    internal bool CanEditForm => Editing is null;

    /// <summary>Title present, and the interval non-empty (all-day: end day ≥ start day).</summary>
    internal bool IsValid
    {
        get
        {
            if (string.IsNullOrWhiteSpace(Title))
            {
                return false;
            }
            return AllDay ? End.Date >= Start.Date : End > Start;
        }
    }

    /// <summary>The create-intent arguments for the current fields.</summary>
    internal CreateArgs CreateArgs()
    {
        if (AllDay)
        {
            return new CreateArgs(
                Title.Trim(),
                DateOnly(Start),
                // The on-screen end day is inclusive; the engine wants the exclusive next day.
                DateOnly(End.AddDays(1)),
                Calendar?.Account,
                Calendar?.Id,
                AllDay: true,
                Timezone: null,
                Notes: string.IsNullOrEmpty(Notes) ? null : Notes,
                Location: string.IsNullOrEmpty(Location) ? null : Location,
                Recurrence: NewRule());
        }
        return new CreateArgs(
            Title.Trim(),
            WallClock(Start),
            WallClock(End),
            Calendar?.Account,
            Calendar?.Id,
            AllDay: false,
            // A wall clock in the device's zone, so the event is created there, not in UTC.
            Timezone: string.IsNullOrEmpty(Zone) ? null : Zone,
            Notes: string.IsNullOrEmpty(Notes) ? null : Notes,
            Location: string.IsNullOrEmpty(Location) ? null : Location,
            Recurrence: NewRule());
    }

    /// <summary>
    /// Whether saving has to ask "this event, or all of them?" first, true exactly when this
    /// editor was opened on one occurrence of a series.
    /// </summary>
    /// <remarks>
    /// A changed repeat settles the question, so it is not put: a rule belongs to the series, and
    /// one occurrence is an instance of a rule rather than a holder of one. The controls say so
    /// before the user touches them.
    /// </remarks>
    internal bool AsksAboutTheSeries =>
        !string.IsNullOrEmpty(Editing?.Occurrence) && RepeatChange is null;

    /// <summary>
    /// Whether the repeat controls are offered at all. An event that does not repeat can always be
    /// given a rule; one that already repeats can only be changed when the core handed over a
    /// draft, which it does not for a rule it could not state in full.
    /// </summary>
    internal bool CanEditRepeat =>
        Editing is null || !Editing.IsRecurring || Editing.SeededRepeat is not null;

    /// <summary>
    /// What this save should send for the repeat rule, or null to leave the series alone. The core
    /// decides it: a repeat changed and changed back is not a change, and the parts no control here
    /// models are put back by the same call.
    /// </summary>
    internal RecurrenceChange? RepeatChange
    {
        get
        {
            if (!CanEditRepeat)
            {
                return null;
            }
            // Nothing chosen on an event that does not repeat. There is no question to put, which
            // is also why a test that says nothing about a repeat needs no native library.
            if (RepeatDraft is null && Editing?.SeededRepeat is null)
            {
                return null;
            }
            return _repeatChangeOf(RepeatDraft, Editing?.SeededRepeat is not null);
        }
    }

    /// <summary>The rule a create sends: whatever the controls hold, as a plain rule rather than
    /// one of the three answers an edit gives.</summary>
    private SimpleRecurrence? NewRule() =>
        RepeatChange is RecurrenceChange.Set set ? set.Rule : null;

    /// <summary>The update-intent arguments for the current fields. Valid only while editing.</summary>
    /// <remarks>
    /// <paramref name="thisOccurrenceOnly"/> splits an override out of the series instead of
    /// rewriting it. Both edges always travel: an occurrence's own times are not the series', so a
    /// single-occurrence edit naming neither would move it onto the master's clock.
    /// </remarks>
    internal UpdateArgs UpdateArgs(bool thisOccurrenceOnly)
    {
        var target = Editing ?? throw new InvalidOperationException("UpdateArgs on a create editor");
        var start = AllDay ? DateOnly(Start) : WallClock(Start);
        var end = AllDay ? DateOnly(End.AddDays(1)) : WallClock(End);
        return new UpdateArgs(
            target.Account,
            target.Key,
            Title.Trim(),
            start,
            end,
            // Empty clears; a value sets.
            Notes: Notes,
            Location: Location,
            Occurrence: thisOccurrenceOnly && !string.IsNullOrEmpty(target.Occurrence)
                ? target.Occurrence
                : null,
            // A rule belongs to the series, so it never travels with an occurrence. The dialog does
            // not offer that combination, and this is the second place it cannot happen.
            Recurrence: thisOccurrenceOnly ? null : RepeatChange);
    }

    /// <summary>The core's own answer. The default everywhere but a test.</summary>
    private static RecurrenceChange? CoreRepeatChangeOf(RepeatDraft? draft, bool wasRepeating) =>
        MailcalBindingsMethods.RepeatChangeOf(draft, wasRepeating);

    /// <summary>A fresh editor: start at the next whole hour, one hour long, in the default calendar.</summary>
    internal static EventEditorState Create(
        CalendarChoice? defaultCalendar,
        string zone,
        DateTime now,
        RepeatChangeOf? repeatChangeOf = null)
    {
        // The next whole hour: +1h, then drop minutes/seconds. Building the DateTime from the parts
        // avoids searching *forward* to the next minute-zero (which from 10:15 lands on 11:00, not
        // 10:00), the same trap the Apple factory guards against.
        var inOneHour = now.AddHours(1);
        var start = new DateTime(
            inOneHour.Year, inOneHour.Month, inOneHour.Day, inOneHour.Hour, 0, 0, DateTimeKind.Unspecified);
        return new EventEditorState(
            editing: null,
            zone: zone,
            title: string.Empty,
            allDay: false,
            start: start,
            end: start.AddHours(1),
            location: string.Empty,
            notes: string.Empty,
            calendar: defaultCalendar,
            repeat: null,
            repeatChangeOf: repeatChangeOf ?? CoreRepeatChangeOf);
    }

    /// <summary>An editor prefilled from a stored event's detail.</summary>
    internal static EventEditorState Edit(
        EventDetail detail,
        string calendarName,
        RepeatChangeOf? repeatChangeOf = null)
    {
        var start = ParseWall(detail.Start);
        // The detail's all-day end is exclusive; show the inclusive last day.
        var end = detail.AllDay ? ParseWall(detail.End).AddDays(-1) : ParseWall(detail.End);
        return new EventEditorState(
            editing: new EditTarget(
                detail.Account,
                detail.Key,
                detail.Timezone,
                detail.IsRecurring,
                detail.ReminderMinutes,
                detail.Recurrence,
                detail.RepeatSummary,
                detail.RepeatDraft,
                detail.OccurrenceStart,
                detail.Attendees),
            zone: detail.Timezone,
            title: detail.Title,
            allDay: detail.AllDay,
            start: start,
            end: end,
            location: detail.Location ?? string.Empty,
            notes: detail.Notes ?? string.Empty,
            calendar: new CalendarChoice(detail.Account, detail.Calendar, calendarName),
            repeat: detail.RepeatDraft,
            repeatChangeOf: repeatChangeOf ?? CoreRepeatChangeOf);
    }

    private static string WallClock(DateTime dt) =>
        dt.ToString("yyyy-MM-ddTHH:mm:ss", CultureInfo.InvariantCulture);

    private static string DateOnly(DateTime dt) =>
        dt.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);

    /// <summary>
    /// Parse <c>YYYY-MM-DDTHH:MM:SS</c> or a bare <c>YYYY-MM-DD</c> (all-day) into a wall clock
    /// (Kind=Unspecified, no zone).
    /// </summary>
    internal static DateTime ParseWall(string value)
    {
        var formats = new[] { "yyyy-MM-ddTHH:mm:ss", "yyyy-MM-dd" };
        if (DateTime.TryParseExact(
                value, formats, CultureInfo.InvariantCulture, DateTimeStyles.None, out var dt))
        {
            return DateTime.SpecifyKind(dt, DateTimeKind.Unspecified);
        }
        return new DateTime(1970, 1, 1, 0, 0, 0, DateTimeKind.Unspecified);
    }
}

/// <summary>A reminder offset, bucketed for display, pure, so a locale quirk can't reach it.</summary>
internal abstract record ReminderBucket
{
    internal sealed record None : ReminderBucket;

    internal sealed record AtStart : ReminderBucket;

    internal sealed record Minutes(int N) : ReminderBucket;

    internal sealed record Hours(int N) : ReminderBucket;

    internal sealed record Days(int N) : ReminderBucket;
}

/// <summary>The reminder summary, as a pure bucket decision (the L10n string assembly lives in the
/// dialog, which is not part of this assembly). The repeat rule's own parts are
/// <see cref="EventRepeatFormat"/>.</summary>
internal static class CalendarEventSummary
{
    /// <summary>Buckets minutes-before into the coarsest exact unit (a day, an hour, else minutes).</summary>
    internal static ReminderBucket ReminderBucketOf(int? minutes)
    {
        if (minutes is not { } m)
        {
            return new ReminderBucket.None();
        }
        if (m <= 0)
        {
            return new ReminderBucket.AtStart();
        }
        if (m % 1440 == 0)
        {
            return new ReminderBucket.Days(m / 1440);
        }
        if (m % 60 == 0)
        {
            return new ReminderBucket.Hours(m / 60);
        }
        return new ReminderBucket.Minutes(m);
    }
}
