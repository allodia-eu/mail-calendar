// The shared relative-timestamp bucketing policy (docs/timestamps.md): today -> the clock, the
// previous six days -> short weekday, this year -> day + month, older -> day + month + year. The
// pattern selection is pure, so the policy Android and macOS re-implement by hand is checkable
// here too; the RelativeDate smoke test pins the day-count + zone conversion against an injected
// "now" so it needs no wall clock.

using System;
using System.Globalization;
using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class RelativeDateTests
{
    [Theory]
    [InlineData(0, true, true, "HH:mm")]
    [InlineData(0, true, false, "h:mm tt")]
    [InlineData(1, true, false, "ddd")]
    [InlineData(6, true, true, "ddd")]
    // Day 7 is the same weekday as today; a bare "Mon" for it would read as *this* Monday.
    [InlineData(7, true, true, "d MMM")]
    [InlineData(200, false, true, "d MMM yyyy")]
    public void PatternBuckets(int dayDiff, bool sameYear, bool use24Hour, string expected) =>
        Assert.Equal(expected, TimeZones.RelativePattern(dayDiff, sameYear, use24Hour));

    [Fact]
    public void TodayRendersTheClockInTheZone()
    {
        // 09:05 UTC, "now" later the same UTC day -> today -> the clock, in UTC.
        var now = DateTimeOffset.Parse("2026-07-20T20:00:00Z", CultureInfo.InvariantCulture);
        Assert.Equal("09:05", TimeZones.RelativeDate("2026-07-20T09:05:00Z", "UTC", true, now));
    }

    [Fact]
    public void AnEarlierDayThisWeekRendersTheWeekday()
    {
        // Three days earlier -> the short weekday (a Friday, 2026-07-17).
        var now = DateTimeOffset.Parse("2026-07-20T20:00:00Z", CultureInfo.InvariantCulture);
        var label = TimeZones.RelativeDate("2026-07-17T09:05:00Z", "UTC", true, now);
        Assert.Equal(
            new DateTime(2026, 7, 17).ToString("ddd", CultureInfo.CurrentCulture),
            label);
    }

    [Fact]
    public void ANaiveValueFallsBackToTheAbsoluteFormat()
    {
        var now = DateTimeOffset.Parse("2026-07-20T20:00:00Z", CultureInfo.InvariantCulture);
        // No trailing Z -> not an instant -> the same rendering LocalDateTime gives.
        Assert.Equal(
            TimeZones.LocalDateTime("2026-07-20T09:05:00", "UTC", true),
            TimeZones.RelativeDate("2026-07-20T09:05:00", "UTC", true, now));
    }

    // The clock setting reaches the row, the reading header and the quote that inherits it, in the
    // calendar's own words, so mail and the grid never disagree about 14:05 and 2:05 PM.
    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public void MailTimesAreDrawnOnTheCalendarsClock(bool use24Hour)
    {
        var now = DateTimeOffset.Parse("2026-07-20T20:00:00Z", CultureInfo.InvariantCulture);
        var clock = CalendarFormat.ClockTime((14 * 60) + 5, use24Hour, CultureInfo.CurrentCulture);

        Assert.Equal(clock, TimeZones.RelativeDate("2026-07-20T14:05:00Z", "UTC", use24Hour, now));
        Assert.Equal(
            "2026-07-20 " + clock,
            TimeZones.LocalDateTime("2026-07-20T14:05:00Z", "UTC", use24Hour));
        // A floating time, an agenda event's, is on the same clock.
        Assert.Equal(
            "2026-07-20 " + clock,
            TimeZones.LocalDateTime("2026-07-20T14:05:00", "UTC", use24Hour));
    }
}
