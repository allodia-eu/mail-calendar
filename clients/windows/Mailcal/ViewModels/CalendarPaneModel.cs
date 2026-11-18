// The calendar pane's own state, and the parts of drawing it that hold without WinUI
// (docs/calendar-pane.md). Everything the pane shows comes from the core's one pull,
// `calendar_pane`; what is kept here is the month the person paged the mini month to, and where the
// grid was when they did. WinUI-free and L10n-free, so Mailcal.Tests holds it: the words arrive as
// parameters.

using System.Globalization;
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.ViewModels;

/// <summary>Where the grid beside the pane is: what the pane is pulled against.</summary>
/// <param name="Layout">What the grid draws.</param>
/// <param name="Anchor">Its first shown day; a day of the month for the month layout.</param>
/// <param name="Columns">How many days it shows; ignored by the core for the month and the agenda.</param>
internal readonly record struct GridPlace(CalendarLayout Layout, DateOnly Anchor, uint Columns);

/// <summary>Which ends of a day's band are rounded within its row (rule 4).</summary>
internal readonly record struct BandEnds(bool RoundStart, bool RoundEnd);

/// <summary>The calendar pane's state: the paged month, and the grid it follows.</summary>
internal sealed class CalendarPaneModel
{
    /// <summary>The mini month's rows (rule 2).</summary>
    internal const int Weeks = 6;

    /// <summary>A day cell is a circle at most this wide, however wide the pane.</summary>
    internal const double MaxCell = 36;

    private GridPlace? _followed;

    /// <summary>The month the person paged to, as the core handed it, or <c>null</c> while the mini
    /// month follows the grid (rule 3).</summary>
    internal string? PagedMonth { get; private set; }

    /// <summary>
    /// The grid is at <paramref name="place"/>. Answers whether its shown days changed, and when they
    /// did lets go of a paged month, so the mini month snaps back to follow the grid.
    /// </summary>
    internal bool Follow(GridPlace place)
    {
        if (_followed == place)
        {
            return false;
        }
        _followed = place;
        PagedMonth = null;
        return true;
    }

    /// <summary>An arrow pressed: <paramref name="month"/> is the <c>previous</c> or <c>next</c> the
    /// core put on the mini month.</summary>
    internal void Page(string month) => PagedMonth = month;

    /// <summary>The pull for the grid at <paramref name="place"/> on <paramref name="today"/>.</summary>
    internal CalendarPaneQuery Query(DateOnly today, GridPlace place) =>
        new(Iso(today), place.Layout, Iso(place.Anchor), place.Columns, PagedMonth);

    /// <summary>The mini month's title, from its first day.</summary>
    internal static string MonthTitle(MiniMonth month, CultureInfo culture) =>
        ParseDate(month.First).ToString("MMMM yyyy", culture);

    /// <summary>The initial heading a weekday's column.</summary>
    internal static string WeekdayInitial(RecurrenceWeekday day, CultureInfo culture)
    {
        var name = culture.DateTimeFormat.ShortestDayNames[EventRepeatFormat.IsoWeekday(day) % 7];
        var first = StringInfo.GetNextTextElementLength(name);
        return first == 0 ? name : name[..first].ToUpper(culture);
    }

    /// <summary>
    /// A day button's spoken name: the full date, then <paramref name="today"/> and
    /// <paramref name="shown"/> where they apply (rule 11).
    /// </summary>
    internal static string DayName(MiniDay day, CultureInfo culture, string today, string shown)
    {
        var name = ParseDate(day.Date).ToString("D", culture);
        if (day.IsToday)
        {
            name += ", " + today;
        }
        if (day.Shown != ShownSpan.Outside)
        {
            name += ", " + shown;
        }
        return name;
    }

    /// <summary>The band a day wears, or <c>null</c> outside it: one capsule per row, so a band
    /// crossing a week boundary is two (rule 4).</summary>
    internal static BandEnds? Band(ShownSpan span) => span switch
    {
        ShownSpan.Only => new BandEnds(true, true),
        ShownSpan.Start => new BandEnds(true, false),
        ShownSpan.Middle => new BandEnds(false, false),
        ShownSpan.End => new BandEnds(false, true),
        _ => null,
    };

    /// <summary>A day cell's side for a pane <paramref name="width"/> wide: an eighth of it, for the
    /// week number and the seven days, and never so wide that a circle stops being round.</summary>
    internal static double CellSize(double width) => Math.Floor(Math.Min(Math.Max(width, 0) / 8, MaxCell));

    /// <summary>
    /// The day an arrow key moves to from the day at <paramref name="index"/> (row by row, seven to a
    /// row): <paramref name="columns"/> across and <paramref name="rows"/> down, staying put at the
    /// mini month's edge.
    /// </summary>
    internal static int Move(int index, int columns, int rows)
    {
        var next = index + columns + (rows * CalendarUnits.DaysInWeek);
        return next is >= 0 and < Weeks * CalendarUnits.DaysInWeek ? next : index;
    }

    /// <summary>The core's <c>YYYY-MM-DD</c>, or <see cref="DateOnly.MinValue"/> for one it never sends.</summary>
    internal static DateOnly ParseDate(string iso) =>
        DateOnly.TryParseExact(iso, "yyyy-MM-dd", CultureInfo.InvariantCulture, DateTimeStyles.None, out var date)
            ? date
            : DateOnly.MinValue;

    private static string Iso(DateOnly date) => date.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);
}
