// Every decision the invitation card makes that is not a pixel, as plain functions over plain
// values, with no WinUI and no L10n in sight.
//
// The core decides *whether there is a card and what is on it* (docs/invitations.md); this file
// decides which sentence applies, which buckets earn a phrase, and how tall the preview's hours are.
// It is the Windows twin of InvitationFormat.swift / InvitationFormat.kt, but where those return
// localised strings, this returns the *choice* and leaves the words to InvitationText (Views/).
// That is not a stylistic difference: Mailcal.Tests is a plain net10.0 assembly and cannot link
// L10n.cs, so a rule phrased as a string here would be a rule no test could reach. Returning the
// bucket is the same seam TimeZones.RelativePattern and CalendarEventSummary already use, and it
// pins the part that can actually be wrong.
//
// Times arrive as UTC instants because the core ships no display tzdata (docs/timestamps.md), so
// every function here takes the display zone as an argument rather than reading a global.
using System;
using System.Collections.Generic;
using System.Globalization;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>A span of wall-clock minutes from midnight, the unit the core's grid solver emits.</summary>
internal readonly record struct MinuteSpan(int Start, int End);

/// <summary>
/// The band of whole hours the meeting-day preview draws: <paramref name="First"/> inclusive,
/// <paramref name="Last"/> <b>exclusive</b>, mirroring the Swift <c>Range&lt;Int&gt;</c> this is a
/// twin of.
/// </summary>
internal readonly record struct HourSpan(int First, int Last)
{
    /// <summary>How many hours the band covers, what the preview divides its height by.</summary>
    internal int Count => Last - First;
}

/// <summary>Which "what else is in your calendar then" sentence applies.</summary>
/// <remarks>
/// <see cref="Unknown"/> is <b>not</b> zero: it means the core could not read the calendar over this
/// window at all, and the count must not be printed (docs/calendar.md §4).
/// </remarks>
internal enum ConflictLine
{
    /// <summary>The calendar was never read, say so; never "nothing else then".</summary>
    Unknown,

    /// <summary>Read, and the window is clear.</summary>
    None,

    /// <summary>Exactly one other commitment, its own sentence, because "1 other things" is not one.</summary>
    One,

    /// <summary>More than one; the count is printed.</summary>
    Many,
}

/// <summary>One phrase of the attendee tally, in the order the card joins them.</summary>
/// <remarks>
/// A <c>…One</c> variant per bucket because the catalog has no plural machinery and Dutch needs a
/// different verb at one ("1 moeten nog antwoorden" is wrong). English reads fine either way, which
/// is exactly why this was invisible until the card was read in Dutch.
/// </remarks>
internal enum AttendeeLine
{
    /// <summary>The only attendee is this account, "1 of 1 accepted" is arithmetic, not a sentence.</summary>
    OnlyYou,

    /// <summary>"N of M accepted", always first.</summary>
    AcceptedOfTotal,

    /// <summary>Exactly one tentative answer.</summary>
    TentativeOne,

    /// <summary>More than one tentative answer.</summary>
    Tentative,

    /// <summary>Exactly one decline.</summary>
    DeclinedOne,

    /// <summary>More than one decline.</summary>
    Declined,

    /// <summary>Exactly one attendee yet to answer.</summary>
    PendingOne,

    /// <summary>More than one attendee yet to answer.</summary>
    Pending,
}

/// <summary>What to say about an answer on its way out, or nothing at all.</summary>
internal enum WriteLine
{
    /// <summary>
    /// Nothing to say. Both <c>Saved</c> and <c>Idle</c> land here on purpose: by then the card
    /// already shows the new answer (it re-reads the calendar), so a second "answer sent" is noise.
    /// </summary>
    None,

    /// <summary>The answer is in flight.</summary>
    Sending,

    /// <summary>
    /// The answer did not go out. The one state that must never be silent, the card would otherwise
    /// sit showing the previous answer while the organiser heard nothing, which is the exact failure
    /// this feature exists to prevent.
    /// </summary>
    Failed,
}

/// <summary>The invitation card's rules, free of words and of pixels.</summary>
internal static class InvitationFormat
{
    /// <summary>The floor on the preview's hour band, see <see cref="PreviewSpan"/>.</summary>
    private const int MinimumPreviewHours = 6;

    /// <summary>Roughly the height one hour label needs before two of them start to collide.</summary>
    private const double LabelHeight = 18;

    /// <summary>
    /// Which conflict sentence the card shows.
    /// </summary>
    /// <remarks>
    /// The count is stated in words rather than left to the preview grid: docs/calendar.md §4, a
    /// picture the user has to read carefully is not a disclosure. And an unread calendar is its own
    /// answer, because "nothing else in your calendar then" over a calendar nobody read is the
    /// confident lie that rule forbids. On a cold start mail syncs before calendars, so an invitation
    /// opened straight away lands exactly there.
    /// </remarks>
    internal static ConflictLine Conflicts(uint count, bool known)
    {
        if (!known)
        {
            return ConflictLine.Unknown;
        }
        return count switch
        {
            0 => ConflictLine.None,
            1 => ConflictLine.One,
            _ => ConflictLine.Many,
        };
    }

    /// <summary>
    /// Which phrases the attendee line is joined from, in order.
    /// </summary>
    /// <remarks>
    /// Counts only, never a roster: the addresses belong to other people and are attacker-controlled
    /// (docs/invitations.md). Every non-zero bucket earns a phrase, because the four sum to the total
    /// and a line that leaves one out reads as arithmetic that does not add up.
    /// </remarks>
    internal static IReadOnlyList<AttendeeLine> Attendees(AttendeeTally tally)
    {
        if (tally.Total == 0)
        {
            return Array.Empty<AttendeeLine>();
        }
        if (tally.Total == 1)
        {
            return new[] { AttendeeLine.OnlyYou };
        }
        var lines = new List<AttendeeLine> { AttendeeLine.AcceptedOfTotal };
        if (tally.Tentative > 0)
        {
            lines.Add(tally.Tentative == 1 ? AttendeeLine.TentativeOne : AttendeeLine.Tentative);
        }
        if (tally.Declined > 0)
        {
            lines.Add(tally.Declined == 1 ? AttendeeLine.DeclinedOne : AttendeeLine.Declined);
        }
        if (tally.NeedsAction > 0)
        {
            lines.Add(tally.NeedsAction == 1 ? AttendeeLine.PendingOne : AttendeeLine.Pending);
        }
        return lines;
    }

    /// <summary>What the respond row says about the write currently settling.</summary>
    internal static WriteLine Write(CalendarWriteStatus status) => status switch
    {
        CalendarWriteStatus.Saving => WriteLine.Sending,
        CalendarWriteStatus.Failed => WriteLine.Failed,
        _ => WriteLine.None,
    };

    /// <summary>
    /// Whether a calendar record is an invitation this account has not answered, the one condition
    /// that turns on the provisional drawing (dashed border, reduced fill).
    /// </summary>
    /// <remarks><c>Declined</c> never reaches a client: the core hides those from every calendar
    /// surface. If one ever did, it is not a hold either.</remarks>
    internal static bool IsAwaitingResponse(ResponseStatus participation) =>
        participation == ResponseStatus.NeedsAction;

    /// <summary>
    /// How much of a hold's colour survives, enough to keep its calendar identifiable, little enough
    /// that it does not read as a confirmed commitment beside one.
    /// </summary>
    internal const double HoldFillAlpha = 0.4;

    /// <summary>
    /// A fill's alpha after the hold treatment: faded on an unanswered invitation, untouched on a
    /// commitment.
    /// </summary>
    /// <remarks>
    /// A byte rather than a <c>Color</c> because this half has to compile without WinUI, the
    /// colour type lives in the Windows projection, which <c>Mailcal.Tests</c> cannot link. The
    /// caller in <c>CalendarHold</c> puts the byte back on the colour.
    /// </remarks>
    internal static byte HoldAlpha(byte alpha, bool awaiting) =>
        awaiting ? (byte)Math.Round(alpha * HoldFillAlpha, MidpointRounding.AwayFromZero) : alpha;

    /// <summary>
    /// A calendar record's spoken label, with the hold said out loud when there is one.
    /// </summary>
    /// <remarks>
    /// The dashed border and hatched gutter that mark an unanswered invitation are **invisible to a
    /// screen reader**, so the label has to say it ([`calendar.md`](../../../docs/calendar.md) §4, the
    /// spoken-grid rule). Shared by the grid block, the all-day bar, the month chip and the agenda row,
    /// so one rule covers every surface that can show a hold.
    /// <para>
    /// Both strings arrive as parameters because they are <c>L10n</c> calls and this file is the
    /// WinUI-free half, the same split as every other rule here.
    /// </para>
    /// </remarks>
    internal static string SpokenWithHold(string label, string awaiting, ResponseStatus participation) =>
        IsAwaitingResponse(participation) ? label + ", " + awaiting : label;

    /// <summary>
    /// The meeting's "when", localised in <paramref name="zone"/>.
    /// </summary>
    /// <remarks>
    /// All-day shows the inclusive day(s), the stored end is exclusive, so a one-day event whose end
    /// is the next midnight must read as one date, not two. A timed meeting collapses the date when
    /// start and end share one. The clock honours the user's 12/24-hour <b>setting</b> rather than
    /// the culture's default (<see cref="CalendarFormat.ClockTime"/>), so mail and calendar cannot
    /// disagree with each other.
    /// <para>Pure: no clock is read, so the same inputs always give the same string.</para>
    /// </remarks>
    internal static string When(
        string startsAt,
        string endsAt,
        bool allDay,
        string zone,
        bool use24Hour,
        CultureInfo culture)
    {
        if (ParseInstant(startsAt) is not { } startUtc)
        {
            return string.Empty;
        }
        var endUtc = ParseInstant(endsAt) ?? startUtc;
        var tz = ZoneOrLocal(zone);
        var start = TimeZoneInfo.ConvertTime(startUtc, tz);
        var end = TimeZoneInfo.ConvertTime(endUtc, tz);

        if (allDay)
        {
            // The stored end is EXCLUSIVE. Naming it would tell the user a one-day event lasts two.
            var lastDay = end.AddDays(-1);
            return lastDay <= start || lastDay.Date == start.Date
                ? start.ToString("D", culture)
                : $"{start.ToString("D", culture)} – {lastDay.ToString("D", culture)}";
        }

        var from = CalendarFormat.ClockTime(MinutesOfDay(start), use24Hour, culture);
        var to = CalendarFormat.ClockTime(MinutesOfDay(end), use24Hour, culture);
        return start.Date == end.Date
            ? $"{start.ToString("D", culture)}, {from} – {to}"
            : $"{start.ToString("D", culture)} {from} – {end.ToString("D", culture)} {to}";
    }

    /// <summary>
    /// The meeting's UTC instants as wall-clock minutes from midnight in <paramref name="zone"/>.
    /// </summary>
    /// <remarks>
    /// Returns a one-hour span at midnight for an instant that will not parse, the preview then
    /// draws the day it was given rather than nothing at all, the same best-effort posture the core
    /// takes when it cannot resolve a conflict window.
    /// </remarks>
    internal static MinuteSpan MeetingMinuteSpan(string startsAt, string endsAt, string zone)
    {
        if (ParseInstant(startsAt) is not { } startUtc)
        {
            return new MinuteSpan(0, 60);
        }
        var endUtc = ParseInstant(endsAt) ?? startUtc;
        var tz = ZoneOrLocal(zone);
        var start = TimeZoneInfo.ConvertTime(startUtc, tz);
        var end = TimeZoneInfo.ConvertTime(endUtc, tz);
        var startMinutes = MinutesOfDay(start);
        // An end past midnight, or on a later day, belongs to the end of this day's grid.
        var endMinutes = start.Date == end.Date
            ? MinutesOfDay(end)
            : CalendarUnits.HoursInDay * 60;
        return new MinuteSpan(startMinutes, Math.Max(endMinutes, startMinutes + 1));
    }

    /// <summary>
    /// The hour band the meeting-day preview draws, in whole hours: <b>the meeting, everything it
    /// clashes with, and an hour of air</b>.
    /// </summary>
    /// <remarks>
    /// Padded a whole hour each side so nothing sits flush against an edge, and never narrower than
    /// <see cref="MinimumPreviewHours"/> so a 30-minute meeting on an empty afternoon still has
    /// context around it.
    /// <para>
    /// It used to span the <b>whole day's</b> blocks, and that is the change: a normal working day
    /// runs 08:00–22:00, so fourteen hours were squeezed into the preview's box and an hour came out
    /// under ten, below <see cref="MinimumTitledHeight"/>, so the invitation's own block drew as an
    /// <i>untitled</i> rectangle beside a titled one. A picture that shows <i>that</i> the afternoon
    /// is taken but not <i>by what</i> answers the wrong question: the reader's next move is deciding
    /// whether the clash matters, and they cannot without the title. Growing the box instead pushed
    /// the message itself off the screen, which is worse, so the band narrows and the hours get
    /// taller.
    /// </para>
    /// <para>
    /// <b>Nothing that the card counts can fall outside this.</b> A conflict is by definition an
    /// event overlapping the meeting's own window, so every one of them widens the band and its
    /// <i>whole</i> extent is inside it, a long booking that starts before the meeting drags
    /// <c>first</c> back with it rather than being drawn cut off at the top edge with its title
    /// off-screen. What is left out is the rest of the day, which the card states in words above the
    /// grid, and which the disclosure label names (<c>invitation_conflicts_preview</c>: "Around this
    /// meeting", not "that day"). docs/calendar.md §4, nothing is hidden without saying so; this
    /// says so.
    /// </para>
    /// </remarks>
    internal static HourSpan PreviewSpan(MinuteSpan meeting, IReadOnlyList<MinuteSpan> others)
    {
        var earliest = meeting.Start;
        var latest = meeting.End;
        foreach (var other in others)
        {
            // Half-open on both sides, exactly as count_conflicts overlaps in the core:
            // back-to-back is not a clash, so an event ending as the meeting starts does not widen
            // the band.
            if (other.Start >= meeting.End || meeting.Start >= other.End)
            {
                continue;
            }
            earliest = Math.Min(earliest, other.Start);
            latest = Math.Max(latest, other.End);
        }
        var first = Math.Max((earliest / 60) - 1, 0);
        // Ceil, so a block ending at 09:15 keeps the whole 09:00 hour, then pad.
        var last = Math.Min(((latest + 59) / 60) + 1, CalendarUnits.HoursInDay);
        // Alternating, later hour first, so the meeting sits near the middle of the band rather than
        // pinned to its top, the hours after a meeting are the more interesting of the two.
        var growAfter = true;
        while (last - first < MinimumPreviewHours && (first > 0 || last < CalendarUnits.HoursInDay))
        {
            if (growAfter && last < CalendarUnits.HoursInDay)
            {
                last++;
            }
            else if (first > 0)
            {
                first--;
            }
            else
            {
                last++;
            }
            growAfter = !growAfter;
        }
        return new HourSpan(first, last);
    }

    /// <summary>
    /// How many hours apart the preview's labelled gridlines sit, given the height one hour gets.
    /// </summary>
    /// <remarks>
    /// A squeezed span leaves no room to label every hour, two labels overlapping is worse than
    /// three-hourly ones, so the stride is derived from the height rather than fixed. Never zero:
    /// a zero stride is a modulo by zero in the ruler.
    /// </remarks>
    internal static int PreviewStride(double hourHeight)
    {
        if (hourHeight <= 0)
        {
            return 1;
        }
        return Math.Max((int)Math.Ceiling(LabelHeight / hourHeight), 1);
    }

    /// <summary>How tall the meeting-day preview draws, for a band of <paramref name="hours"/>.</summary>
    /// <remarks>
    /// <b>Normally just <see cref="OrdinaryPreviewHeight"/></b>, the band is narrow now
    /// (<see cref="PreviewSpan"/> shows the meeting and its clashes, not the whole day), so at six
    /// hours an hour already gets 22 and there is nothing to fix. This exists for the case the band
    /// <i>cannot</i> be narrow: an all-morning booking the meeting sits inside drags the band out to
    /// ten or twelve hours, and at a fixed height the blocks around it would go back to being
    /// untitled rectangles. So an hour is allowed <see cref="IdealPreviewHourHeight"/> and the box
    /// grows, up to <see cref="MaximumPreviewHeight"/>, past which this stops being a preview
    /// sitting above a message and starts pushing the message off the screen.
    /// <para>
    /// Beyond that cap the hour height falls back below the ideal and short blocks quietly lose
    /// their titles. That is the correct trade and not a hole in the rule above: nothing is ever
    /// <i>clipped</i>, only untitled, and every block keeps its spoken label (docs/calendar.md §4).
    /// </para>
    /// <para>
    /// The three numbers are <i>layout</i>, and a platform may hold its own; the formula is the
    /// rule, and it is the same in InvitationFormat.swift and InvitationFormat.kt.
    /// </para>
    /// </remarks>
    internal static double PreviewHeight(int hours) => Math.Clamp(
        Math.Max(hours, 1) * IdealPreviewHourHeight,
        OrdinaryPreviewHeight,
        MaximumPreviewHeight);

    /// <summary>
    /// Below this a preview block gets no title rather than one sliced through the middle, the
    /// same rule the full grid applies at a low zoom.
    /// </summary>
    /// <remarks>
    /// Here rather than beside the chip that reads it, because <c>Mailcal.Tests</c> is a plain
    /// net10.0 assembly and cannot link a WinUI file: a threshold declared in Views/ is a threshold
    /// no test can compose with <see cref="PreviewHeight"/>, and the two only mean anything
    /// together.
    /// </remarks>
    internal const double MinimumTitledHeight = 12;

    /// <summary>The height one hour wants: room for a 60-minute block's title plus its insets.</summary>
    private const double IdealPreviewHourHeight = 20;

    /// <summary>What the preview normally is, short enough that the message body is still on screen.</summary>
    private const double OrdinaryPreviewHeight = 132;

    /// <summary>The ceiling, for a band a long booking forced wide. Taller than this is not a preview.</summary>
    private const double MaximumPreviewHeight = 240;

    /// <summary>The core's RFC 3339 UTC instant, or null when it will not parse.</summary>
    private static DateTimeOffset? ParseInstant(string raw) =>
        DateTimeOffset.TryParse(
            raw,
            CultureInfo.InvariantCulture,
            DateTimeStyles.AssumeUniversal | DateTimeStyles.AdjustToUniversal,
            out var parsed)
            ? parsed
            : null;

    // The display zone, or the device's when the core named one this host cannot resolve. Falling
    // back keeps the card drawn (in the wrong zone, visibly) rather than blank.
    private static TimeZoneInfo ZoneOrLocal(string zone)
    {
        try
        {
            return TimeZoneInfo.FindSystemTimeZoneById(zone);
        }
        catch (Exception ex) when (
            ex is TimeZoneNotFoundException or InvalidTimeZoneException or ArgumentException)
        {
            // An empty zone reaches here too: the preview carries none until the core has laid a day
            // out, and the card is opened before that on a cold start.
            return TimeZoneInfo.Local;
        }
    }

    private static int MinutesOfDay(DateTimeOffset at) => (at.Hour * 60) + at.Minute;
}
