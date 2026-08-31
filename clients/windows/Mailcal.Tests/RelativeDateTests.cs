// The shared relative-timestamp bucketing policy (docs/timestamps.md): today -> the clock, the
// previous six days -> short weekday, this year -> day + month, older -> day + month + year. The
// pattern selection is pure, so the policy Android and macOS re-implement by hand is checkable
// here too; the RelativeDate smoke test pins the day-count + zone conversion against an injected
// "now" so it needs no wall clock.

using System;
using System.Globalization;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class RelativeDateTests
{
    [Theory]
    [InlineData(0, true, "HH:mm")]
    [InlineData(1, true, "ddd")]
    [InlineData(6, true, "ddd")]
    // Day 7 is the same weekday as today; a bare "Mon" for it would read as *this* Monday.
    [InlineData(7, true, "d MMM")]
    [InlineData(200, false, "d MMM yyyy")]
    public void PatternBuckets(int dayDiff, bool sameYear, string expected) =>
        Assert.Equal(expected, TimeZones.RelativePattern(dayDiff, sameYear));

    [Fact]
    public void TodayRendersTheClockInTheZone()
    {
        // 09:05 UTC, "now" later the same UTC day -> today -> the clock, in UTC.
        var now = DateTimeOffset.Parse("2026-07-20T20:00:00Z", CultureInfo.InvariantCulture);
        Assert.Equal("09:05", TimeZones.RelativeDate("2026-07-20T09:05:00Z", "UTC", now));
    }

    [Fact]
    public void AnEarlierDayThisWeekRendersTheWeekday()
    {
        // Three days earlier -> the short weekday (a Friday, 2026-07-17).
        var now = DateTimeOffset.Parse("2026-07-20T20:00:00Z", CultureInfo.InvariantCulture);
        var label = TimeZones.RelativeDate("2026-07-17T09:05:00Z", "UTC", now);
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
            TimeZones.LocalDateTime("2026-07-20T09:05:00", "UTC"),
            TimeZones.RelativeDate("2026-07-20T09:05:00", "UTC", now));
    }
}
