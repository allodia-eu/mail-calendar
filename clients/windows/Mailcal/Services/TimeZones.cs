// A zone-aware timestamp formatter, the Windows counterpart of macOS's
// TimeZoneViews.swift localDateTime. The view-model is tzdata-free (the engine resolves
// instants; the host localises them), so this host-side conversion is where "shown in your
// chosen time zone" happens. .NET 10 on Windows uses ICU, so IANA ids resolve directly.
//
// Device-zone detection and the picker's zone list are NOT here: both come from shared Rust
// over the FFI (MailcalBindingsMethods.DeviceTimeZone / .AvailableTimeZones) so they are
// region-aware and consistent across platforms, see MailboxModel.

using System.Globalization;
using Allodia.Mailcal.Calendar;

namespace Allodia.Mailcal.Services;

/// <summary>IANA-zone timestamp formatting for display.</summary>
internal static class TimeZones
{
    /// <summary>
    /// Formats an engine timestamp for display in <paramref name="zone"/> (an IANA id). A
    /// <c>Z</c>-suffixed UTC instant (mail received_at, a resolved event start) is converted
    /// to the zone; a naive wall-clock is shown as-is; a bare date is shown as the date. The time
    /// is on <paramref name="use24Hour"/>'s clock, drawn as the calendar draws it.
    /// </summary>
    public static string LocalDateTime(string raw, string zone, bool use24Hour) =>
        LocalDateTime(raw, zone, use24Hour, CultureInfo.CurrentCulture);

    internal static string LocalDateTime(string raw, string zone, bool use24Hour, CultureInfo culture)
    {
        if (string.IsNullOrEmpty(raw))
        {
            return "";
        }
        if (raw.EndsWith('Z')
            && DateTimeOffset.TryParse(raw, CultureInfo.InvariantCulture,
                DateTimeStyles.AssumeUniversal | DateTimeStyles.AdjustToUniversal, out var utc))
        {
            try
            {
                var tz = TimeZoneInfo.FindSystemTimeZoneById(zone);
                var local = TimeZoneInfo.ConvertTime(utc, tz);
                return local.ToString("yyyy-MM-dd ", CultureInfo.InvariantCulture)
                    + CalendarFormat.ClockTime((local.Hour * 60) + local.Minute, use24Hour, culture);
            }
            catch (TimeZoneNotFoundException)
            {
                // Fall through to the naive rendering below.
            }
        }
        // A naive wall-clock "YYYY-MM-DDTHH:MM:SS" -> its date and clock as it stands; else a bare
        // date.
        if (raw.Length >= 16 && raw.Contains('T')
            && int.TryParse(raw.AsSpan(11, 2), NumberStyles.None, CultureInfo.InvariantCulture, out var hour)
            && int.TryParse(raw.AsSpan(14, 2), NumberStyles.None, CultureInfo.InvariantCulture, out var minute))
        {
            return raw[..10] + " " + CalendarFormat.ClockTime((hour * 60) + minute, use24Hour, culture);
        }
        return raw.Length >= 10 ? raw[..10] : raw;
    }

    /// <summary>
    /// A compact, Thunderbird-style relative label for a list row, in <paramref name="zone"/>:
    /// today -> time, the previous six days -> short weekday, this year -> day + month, older ->
    /// with the year. Falls back to <see cref="LocalDateTime(string, string, bool)"/> for a
    /// naive/unparseable value; the reading header keeps the full
    /// <see cref="LocalDateTime(string, string, bool)"/>. Mirrors Android's and macOS's relativeDate
    /// (docs/timestamps.md).
    /// </summary>
    public static string RelativeDate(string raw, string zone, bool use24Hour) =>
        RelativeDate(raw, zone, use24Hour, DateTimeOffset.UtcNow, CultureInfo.CurrentCulture);

    // The now-injectable core, so the bucketing is deterministically testable without the clock.
    // Keeps the ambient culture, which AppCulture has pinned to the app's language choice.
    internal static string RelativeDate(string raw, string zone, bool use24Hour, DateTimeOffset now) =>
        RelativeDate(raw, zone, use24Hour, now, CultureInfo.CurrentCulture);

    // Culture-injectable as well as now-injectable: the weekday bucket is the one part of this
    // label that is *language*, and asserting it against CultureInfo.CurrentCulture, as the
    // bucketing tests necessarily do, passes in every culture and so cannot fail when the app's
    // language and its formatting culture disagree. Taking the culture as an argument is what lets
    // a test pin "ma" rather than merely "whatever this machine calls Monday".
    internal static string RelativeDate(
        string raw, string zone, bool use24Hour, DateTimeOffset now, CultureInfo culture)
    {
        if (string.IsNullOrEmpty(raw) || !raw.EndsWith('Z')
            || !DateTimeOffset.TryParse(raw, CultureInfo.InvariantCulture,
                DateTimeStyles.AssumeUniversal | DateTimeStyles.AdjustToUniversal, out var utc))
        {
            return LocalDateTime(raw, zone, use24Hour, culture);
        }
        TimeZoneInfo tz;
        try
        {
            tz = TimeZoneInfo.FindSystemTimeZoneById(zone);
        }
        catch (TimeZoneNotFoundException)
        {
            return LocalDateTime(raw, zone, use24Hour, culture);
        }
        var local = TimeZoneInfo.ConvertTime(utc, tz);
        var localNow = TimeZoneInfo.ConvertTime(now, tz);
        // Compare calendar-day parts in the target zone, so the day count is DST-safe.
        var dayDiff = (localNow.Date - local.Date).Days;
        var sameYear = local.Year == localNow.Year;
        return local.ToString(RelativePattern(dayDiff, sameYear, use24Hour), culture);
    }

    /// <summary>
    /// The custom format the relative label uses for a message <paramref name="dayDiff"/> calendar
    /// days in the past (0 = today), in <paramref name="sameYear"/> as now. Day 7 falls to the date
    /// on purpose, it is the same weekday as today. Pure, so the shared bucketing policy is
    /// unit-testable (docs/timestamps.md), mirrored on Android and macOS.
    /// </summary>
    internal static string RelativePattern(int dayDiff, bool sameYear, bool use24Hour) => dayDiff switch
    {
        0 => ClockPattern(use24Hour),
        >= 1 and <= 6 => "ddd",
        _ => sameYear ? "d MMM" : "d MMM yyyy",
    };

    // The today label's clock, as CalendarFormat.ClockTime draws it; RelativeDateTests holds the two
    // together.
    private static string ClockPattern(bool use24Hour) => use24Hour ? "HH:mm" : "h:mm tt";
}
