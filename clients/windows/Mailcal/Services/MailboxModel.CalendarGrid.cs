// The time grid's half of the model, and it does NOT look like the rest of this class.
//
// Every other surface follows the mail pattern: mutate -> observer.surface_changed -> the host pulls
// one immutable snapshot. **That breaks under a pager** (docs/calendar.md §5):
//
//   - the grid holds FIVE pages at once (the one in view, and two either side, because a banked week
//     may lag the pixels by two). One snapshot slot cannot hold five.
//   - `dispatch` is fire-and-forget on a multi-threaded runtime, so two quick swipes RACE: the grid
//     can settle on last week after the user has already swiped to next.
//   - the observer is debounced at 250ms.
//
// So `calendar_range` / `month_page` are direct, synchronous, argument-taking queries over an
// in-memory cache in the core. They never touch the store or the network. **The client owns the
// anchor; the core never learns where the user is.** A pull cannot arrive out of order.
//
// `Surface.Calendar` survives, demoted to a cache-invalidation signal: "calendar data changed,
// re-pull whatever you are showing."
using System;
using System.Globalization;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private int _calendarVersion;

    /// <summary>
    /// Bumped whenever the core says its calendar data changed. The grid watches this and re-pulls
    /// the pages it is showing; it is <b>not</b> a snapshot to read.
    /// </summary>
    public int CalendarVersion
    {
        get => _calendarVersion;
        private set => Set(ref _calendarVersion, value);
    }

    private int _displaySettingsVersion;

    /// <summary>
    /// Bumped whenever the core signals a <b>settings</b> change. Distinct from
    /// <see cref="CalendarVersion"/>, which fires on every mailbox refresh: the calendar re-pulls its
    /// pages on the latter, but must only re-apply the display settings (horizon, clock, week-start
    /// alignment), and re-seat the grid, on <i>this</i> one. Re-seating on every mailbox refresh
    /// would jerk the grid back to today whenever a background sync landed.
    /// </summary>
    public int DisplaySettingsVersion
    {
        get => _displaySettingsVersion;
        private set => Set(ref _displaySettingsVersion, value);
    }

    /// <summary>Marks the display settings stale, so the calendar re-applies them (Surface.Settings).</summary>
    internal void BumpDisplaySettings() => DisplaySettingsVersion++;

    /// <summary>
    /// The grid's page query: <paramref name="columns"/> consecutive days from <paramref name="from"/>.
    /// </summary>
    /// <remarks>
    /// Synchronous and cheap, an in-memory read, never the store and never the network, so the grid
    /// calls it for the page in view <i>and</i> its neighbours, and may call it again on any frame.
    /// <para>
    /// The week's first day is deliberately <b>not</b> passed: that is a core setting the core
    /// applies. And the range is snapped to nothing, which is what lets a zoom widen three columns to
    /// seven without relocating the grid (§2).
    /// </para>
    /// </remarks>
    internal CalendarPage? CalendarRange(DateOnly from, uint columns) =>
        _app?.CalendarRange(Iso(from), columns);

    /// <summary>The month grid, a different query, not the time grid with more columns.</summary>
    internal MonthPage? MonthPage(DateOnly anchor) => _app?.MonthPage(Iso(anchor));

    /// <summary>
    /// One event's full detail, the read a tap opens, and what the editor prefills from.
    /// </summary>
    /// <remarks>
    /// A direct, synchronous read over the same in-memory store the grid pulls from (never the network),
    /// so a tap can open the detail on the spot. <c>null</c> when the event is gone, the store changed
    /// under a stale snapshot, in which case the caller simply does not open the sheet.
    /// </remarks>
    /// <remarks>
    /// <paramref name="occurrence"/> is the token the tapped surface carried, passed back verbatim
    /// so the times are that occurrence's rather than the series', a series' own start is its
    /// <em>first</em> occurrence's. Empty for an agenda row and a one-off event.
    /// </remarks>
    internal EventDetail? EventDetail(string account, string key, string occurrence) =>
        _app?.EventDetail(account, key, occurrence);

    /// <summary>
    /// Every calendar across every account, for the manager, read off a page pull.
    /// </summary>
    /// <remarks>
    /// There is no dedicated enumeration query: the calendar list (with each one's resolved colour and
    /// visibility) rides on every page, exactly as Android reads it off the current page. Any range
    /// returns the same full set, so this pulls a cheap in-memory week and takes its <c>calendars</c>.
    /// </remarks>
    internal CalendarRow[] Calendars()
    {
        var today = DateOnly.FromDateTime(DateTime.Now);
        return CalendarRange(WeekStart(today), 7)?.Calendars ?? [];
    }

    /// <summary>
    /// The first day of <paramref name="date"/>'s week, <b>from the core</b>.
    /// </summary>
    /// <remarks>
    /// Never derived from the device locale here. The core owns <c>WeekStart</c> (a persisted setting,
    /// defaulting to Monday), and deriving it client-side is how the two drift apart, at which point
    /// every column of the grid shifts and the user reads Tuesday's meetings under Monday's heading
    /// (§3).
    /// </remarks>
    internal DateOnly WeekStart(DateOnly date)
    {
        var iso = _app?.WeekStartDate(Iso(date));
        return iso is null
            ? date
            : DateOnly.TryParseExact(iso, "yyyy-MM-dd", CultureInfo.InvariantCulture, DateTimeStyles.None, out var d)
                ? d
                : date;
    }

    /// <summary>The persisted display settings: week start, 12/24h, horizon, shape.</summary>
    internal DisplaySettings? CalendarDisplaySettings() => _app?.DisplaySettings();

    /// <summary>
    /// Persists the first day of the week.
    /// </summary>
    /// <remarks>
    /// The core signals both <c>Settings</c> and <c>Calendar</c>, the grid's columns are laid out
    /// around this, so every page a client is showing is stale and must be re-pulled and re-aligned
    /// (§3). Owned by the core, never derived from the device locale here.
    /// </remarks>
    internal void SetWeekStart(WeekStart start)
    {
        Log.Info($"cal: set week_start={start}");
        _app?.SetWeekStart(start);
    }

    /// <summary>Persists the 12/24-hour clock, for mail <b>and</b> calendar alike, one app must not
    /// disagree with itself about whether it is <c>14:05</c> or <c>2:05 PM</c>.</summary>
    internal void SetTimeFormat(TimeFormat format)
    {
        Log.Info($"cal: set time_format={format}");
        _app?.SetTimeFormat(format);
    }

    /// <summary>
    /// Persists the horizon a settled pinch landed on.
    /// </summary>
    /// <remarks>
    /// Persisted in the <b>core</b>, not the client, and clamped there, a pinch runs off the end of
    /// its own gesture constantly, and a client that sent the raw value would leave one platform
    /// showing a 1-hour day. It is also what stops the phone and the desktop opening on different
    /// calendars (§8).
    /// </remarks>
    internal void SetCalendarVisibleHours(int hours)
    {
        var clamped = (byte)Math.Clamp(hours, 4, 24);
        Log.Info($"cal: set visible_hours={clamped}");
        _app?.SetCalendarVisibleHours(clamped);
    }

    /// <summary>Persists the shape a settled pinch (or a menu choice) landed on.</summary>
    internal void SetCalendarLayout(CalendarLayout layout)
    {
        Log.Info($"cal: set layout={layout}");
        _app?.SetCalendarLayout(layout);
    }

    /// <summary>Per-calendar visibility, applied at page-pull time, so no sync and no network.</summary>
    /// <remarks>Logs the calendar's provider key (an id, which the logging contract permits) and never
    /// the account id, which embeds the address (docs/logging.md).</remarks>
    internal void SetCalendarVisible(string account, string calendar, bool visible)
    {
        Log.Info($"cal: calendar visible={visible} id={calendar}");
        _app?.SetCalendarVisible(account, calendar, visible);
    }

    /// <summary>Per-calendar colour override. <c>null</c> restores the server's own colour.</summary>
    internal void SetCalendarColor(string account, string calendar, string? hex)
    {
        Log.Info($"cal: calendar colour {(hex is null ? "reset" : "set")} id={calendar}");
        _app?.SetCalendarColor(account, calendar, hex);
    }

    /// <summary>The ten calendar colours the core will accept. Allodia Orange is deliberately absent,
    /// it means "action".</summary>
    internal static string[] CalendarPalette() => MailcalBindingsMethods.CalendarPalette();

    private static string Iso(DateOnly date) => date.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);
}
