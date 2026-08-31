// One week's page, reduced to exactly what a draw call needs, and nothing a zoom can change.
//
// This file is §7 of the calendar contract made structural rather than merely disciplined. A pinch
// moves the hour height on **every frame**, and Android's first grid rebuilt, per event, per frame:
// three hex colours parsed, a clock formatted, and a localised accessibility string assembled out of
// resources. All of it far more expensive than the arithmetic it sat next to, and none of it able to
// change when only the zoom does.
//
// So the split is enforced by the types. Everything here is derived ONCE, when the page (or the
// theme, or the clock format, or the locale) changes. What is left for the renderer is a day index,
// a wall-clock minute and a column fraction, the core's unit-free geometry, multiplied by an hour
// height and a column width. **Multiplication is all a frame is allowed to do.**
using System;
using System.Collections.Generic;
using System.Globalization;
using Windows.UI;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>A column heading: the weekday and the date, already localised.</summary>
/// <remarks>
/// The core emits an ISO date and owns no locale facility at all (AGENTS.md: "Localisation is
/// client-side"), so the short weekday is assembled here, once per page, not once per frame.
/// </remarks>
internal sealed record DayHeading(DateOnly Date, string Weekday, string DayOfMonth);

/// <summary>
/// One timed event, ready to draw.
/// </summary>
/// <remarks>
/// <see cref="Span"/> is the core's geometry, untouched: a day index, two wall-clock minutes, and
/// this event's share of its day. The renderer multiplies it by the zoom and does not otherwise
/// think. The colours are already <see cref="Color"/>s, parsing a hex string per block per frame is
/// exactly the cost this type exists to remove.
/// </remarks>
internal sealed record BlockPaint(
    BlockSpan Span,
    string Title,
    string Clock,
    /// <summary>Unconditional: a block too short to show its title still speaks it.</summary>
    string Spoken,
    Color Background,
    Color Border,
    Color Text,
    /// <summary>The owning account and the event's provider key, what a tap opens the detail for.
    /// Carried alongside the geometry so a hit maps straight back to an <c>EventRef</c> (§7).</summary>
    string Account,
    string Event,
    /// <summary>This occurrence's own start, as the core minted it, and empty when the event does
    /// not recur, the token that names one occurrence to a write (§10). Carried here because the
    /// detail a tap opens describes the <i>series</i>: by then, which day was drawn is gone.</summary>
    string OccurrenceStart,
    /// <summary>
    /// An invitation this account has not answered, drawn provisionally rather than as a booked
    /// commitment (<see cref="CalendarHold"/>, docs/invitations.md).
    /// </summary>
    /// <remarks>
    /// Resolved <b>here</b>, with the colours and the strings, and never inside a frame: a pinch may
    /// not re-derive what kind of record this is any more than it may re-parse a hex colour (§7).
    /// </remarks>
    bool Awaiting)
{
    internal int Minutes => Span.Minutes;
}

/// <summary>One all-day or multi-day bar, ready to draw. <see cref="Span"/> is the core's stacking.</summary>
internal sealed record BandPaint(
    BandSpan Span,
    string Title,
    string Spoken,
    Color Background,
    Color Text,
    /// <summary>The owning account and the event's provider key, what a tap opens the detail for.</summary>
    string Account,
    string Event,
    /// <summary>As <see cref="BlockPaint.OccurrenceStart"/>: this bar's own occurrence, and empty
    /// when the event does not recur.</summary>
    string OccurrenceStart,
    /// <summary>The edge a hold is dashed and hatched with. A bar draws no border otherwise, so this
    /// is used only when <see cref="Awaiting"/>.</summary>
    Color Border,
    /// <summary>As <see cref="BlockPaint.Awaiting"/>: an unanswered hold.</summary>
    bool Awaiting);

/// <summary>
/// A whole week, drawable. Built once per (page, theme, clock format, locale) and then held across
/// every frame of a pinch and every pixel of a swipe.
/// </summary>
internal sealed record PagePaint(
    IReadOnlyList<DayHeading> Headings,
    int WeekNumber,
    string WeekSpoken,
    IReadOnlyList<BlockPaint> Blocks,
    IReadOnlyList<BandPaint> Bands,
    /// <summary>The true lane count the core stacked the bands into, <b>not</b> what the banner shows.</summary>
    int Lanes,
    /// <summary>
    /// What each day column's collapsed banner is hiding, and the chip that says so.
    /// </summary>
    /// <remarks>
    /// Precomputed for the <b>collapsed</b> banner only, because an expanded one hides nothing by
    /// definition, so this never depends on the banner's live state, and never costs a frame.
    /// <para>
    /// The counts are per column, and a hidden multi-day bar counts against <i>every</i> day it
    /// covers: a three-day offsite pushed out of view adds one to three different columns. A "+1"
    /// that should say "+2" is a lie the user cannot see through.
    /// </para>
    /// </remarks>
    IReadOnlyList<string> MoreLabels,
    IReadOnlyList<string> MoreSpoken,
    /// <summary>
    /// <b><c>false</c> does not mean "no events".</b> It means the engine has not expanded this far
    /// yet, and the page must say so in words rather than render a confidently empty week (§4).
    /// </summary>
    bool IsMaterialized)
{
    /// <summary>An empty page, for a week the core has not answered for yet.</summary>
    /// <remarks>
    /// <c>IsMaterialized = false</c>, deliberately: a week we have not asked about is a week we have
    /// not looked at, and it must say "loading" rather than draw a confidently empty grid.
    /// </remarks>
    internal static readonly PagePaint Empty = new(
        [], 0, string.Empty, [], [], 0, [], [], IsMaterialized: false);

    internal IReadOnlyList<DateOnly> Days
    {
        get
        {
            var days = new DateOnly[Headings.Count];
            for (var i = 0; i < Headings.Count; i++)
            {
                days[i] = Headings[i].Date;
            }
            return days;
        }
    }
}

/// <summary>Turns one of the core's pages into everything a frame needs.</summary>
internal static class CalendarPaint
{
    /// <summary>
    /// Builds a drawable page from the core's <see cref="CalendarPage"/>.
    /// </summary>
    /// <remarks>
    /// Every string is formatted, every colour parsed, and every spoken label assembled <b>here</b>,
    /// once. Called when the page, the theme, the clock format or the locale changes, and on nothing
    /// else. A zoom is deliberately not in that list, which is the whole point: pinching cannot
    /// invalidate this.
    /// </remarks>
    internal static PagePaint ToPaint(
        this CalendarPage page,
        bool dark,
        bool use24Hour,
        CultureInfo culture)
    {
        var calendars = new Dictionary<string, CalendarRow>(StringComparer.Ordinal);
        foreach (var row in page.Calendars)
        {
            calendars[Key(row.Account, row.Id)] = row;
        }

        var headings = new List<DayHeading>(page.Days.Length);
        foreach (var day in page.Days)
        {
            var date = ParseDate(day.Date);
            headings.Add(new DayHeading(
                date,
                culture.DateTimeFormat.AbbreviatedDayNames[(int)date.DayOfWeek],
                date.Day.ToString(CultureInfo.CurrentCulture)));
        }

        var blocks = new List<BlockPaint>(page.Timed.Length);
        foreach (var seg in page.Timed)
        {
            var swatch = SwatchFor(calendars, seg.Account, seg.Calendar, dark);
            var clock = CalendarFormat.TimeRange(seg.StartMinutes, seg.EndMinutes, use24Hour, culture);
            var awaiting = InvitationFormat.IsAwaitingResponse(seg.Participation);
            blocks.Add(new BlockPaint(
                new BlockSpan(
                    (int)seg.Day,
                    (int)seg.Column,
                    (int)Math.Max(seg.Columns, 1),
                    (int)seg.StartMinutes,
                    (int)seg.EndMinutes),
                seg.Title,
                clock,
                Spoken(seg.Title, clock, CalendarName(calendars, seg.Account, seg.Calendar), seg.Participation),
                CalendarHold.Fade(Parse(swatch.Background), awaiting),
                Parse(swatch.Border),
                Parse(swatch.Text),
                seg.Account,
                seg.Event,
                seg.OccurrenceStart,
                awaiting));
        }

        var bands = new List<BandPaint>(page.AllDay.Length);
        var spans = new List<BandSpan>(page.AllDay.Length);
        foreach (var band in page.AllDay)
        {
            var swatch = SwatchFor(calendars, band.Account, band.Calendar, dark);
            var span = new BandSpan((int)band.Day, (int)Math.Max(band.Days, 1), (int)band.Lane);
            spans.Add(span);
            var awaiting = InvitationFormat.IsAwaitingResponse(band.Participation);
            bands.Add(new BandPaint(
                span,
                band.Title,
                Spoken(
                    band.Title,
                    L10n.CalendarAllDay(),
                    CalendarName(calendars, band.Account, band.Calendar),
                    band.Participation),
                CalendarHold.Fade(Parse(swatch.Background), awaiting),
                Parse(swatch.Text),
                band.Account,
                band.Event,
                band.OccurrenceStart,
                Parse(swatch.Border),
                awaiting));
        }

        var lanes = (int)page.AllDayLanes;
        var drawn = CalendarAllDay.DrawnLanes(lanes, expanded: false);
        var hidden = CalendarAllDay.OverflowPerDay(spans, page.Days.Length, drawn);

        var moreLabels = new string[page.Days.Length];
        var moreSpoken = new string[page.Days.Length];
        for (var day = 0; day < page.Days.Length; day++)
        {
            var n = hidden[day];
            moreLabels[day] = n > 0 ? L10n.CalendarAllDayMore(n) : string.Empty;
            moreSpoken[day] = n > 0 ? L10n.CalendarAllDayExpand(n) : string.Empty;
        }

        var week = headings.Count > 0 ? IsoWeek(headings[0].Date) : 0;

        return new PagePaint(
            headings,
            week,
            week > 0 ? L10n.CalendarWeekNumber(week.ToString(CultureInfo.CurrentCulture)) : string.Empty,
            blocks,
            bands,
            lanes,
            moreLabels,
            moreSpoken,
            page.IsMaterialized);
    }

    /// <summary>The ISO-8601 week number, the one the header shows.</summary>
    internal static int IsoWeek(DateOnly date) =>
        ISOWeek.GetWeekOfYear(date.ToDateTime(TimeOnly.MinValue));

    /// <summary>
    /// The event's spoken label: title, time, calendar, and the hold, when there is one. Never
    /// truncated by the zoom (§4).
    /// </summary>
    /// <remarks>
    /// The dashed border and hatched gutter <see cref="CalendarHold"/> draws are invisible to a
    /// screen reader, so the label has to carry the same fact in words. Assembled here, once per
    /// page, like every other string on a <see cref="BlockPaint"/>.
    /// </remarks>
    private static string Spoken(
        string title, string time, string calendar, ResponseStatus participation) =>
        InvitationFormat.SpokenWithHold(
            L10n.CalendarEventA11y(title, time, calendar),
            L10n.A11yInvitationAwaitingResponse(),
            participation);

    private static string Key(string account, string calendar) => account + " " + calendar;

    private static string CalendarName(
        Dictionary<string, CalendarRow> calendars,
        string account,
        string calendar) =>
        calendars.TryGetValue(Key(account, calendar), out var row) ? row.Name : string.Empty;

    /// <summary>
    /// The calendar's swatch for the current theme, <b>resolved by the core</b>, which guarantees the
    /// label reads at ≥ 4.5:1 against its fill. No client computes contrast, so none of them can
    /// disagree about whether a chip is readable (§1).
    /// </summary>
    private static Swatch SwatchFor(
        Dictionary<string, CalendarRow> calendars,
        string account,
        string calendar,
        bool dark)
    {
        if (calendars.TryGetValue(Key(account, calendar), out var row))
        {
            return dark ? row.Color.Dark : row.Color.Light;
        }
        // A calendar the page did not name. Grey, and still legible, never an invisible event.
        return new Swatch("#5a5a5a", "#ffffff", "#3c3c3c");
    }

    /// <summary>Parses a <c>#rrggbb</c> from the core. Opaque: the core never sends an alpha.</summary>
    private static Color Parse(string hex)
    {
        if (hex.Length != 7 || hex[0] != '#' ||
            !byte.TryParse(hex.AsSpan(1, 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var r) ||
            !byte.TryParse(hex.AsSpan(3, 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var g) ||
            !byte.TryParse(hex.AsSpan(5, 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var b))
        {
            return Color.FromArgb(255, 90, 90, 90);
        }
        return Color.FromArgb(255, r, g, b);
    }

    /// <summary>
    /// Parses the core's <c>YYYY-MM-DD</c>.
    /// </summary>
    /// <remarks>
    /// Falls back to today rather than throwing: the core already promises a valid date, and a grid
    /// that crashes on a malformed one is strictly worse than a grid that draws the wrong week.
    /// </remarks>
    internal static DateOnly ParseDate(string iso) =>
        DateOnly.TryParseExact(iso, "yyyy-MM-dd", CultureInfo.InvariantCulture, DateTimeStyles.None, out var d)
            ? d
            : DateOnly.FromDateTime(DateTime.Now);
}
