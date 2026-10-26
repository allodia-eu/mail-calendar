// The calendar write-status badge's mapping from the core's CalendarWriteStatus.
//
// The mapping is the whole point that can go wrong client-side: the core decides the status; this side
// only turns it into what the header shows. A pure test pins it without a UI, a rendered badge cannot
// tell you the mapping is right, only that it did not throw. Mirrors the Android and Apple suites.
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarWriteIndicatorTests
{
    [Fact]
    public void Every_status_maps_to_an_indicator()
    {
        Assert.Equal(CalendarWriteIndicator.Hidden, CalendarWriteIndicators.Of(CalendarWriteStatus.Idle));
        Assert.Equal(CalendarWriteIndicator.Spinner, CalendarWriteIndicators.Of(CalendarWriteStatus.Saving));
        Assert.Equal(CalendarWriteIndicator.Saved, CalendarWriteIndicators.Of(CalendarWriteStatus.Saved));
        Assert.Equal(CalendarWriteIndicator.Warning, CalendarWriteIndicators.Of(CalendarWriteStatus.NotSaved));
        Assert.Equal(CalendarWriteIndicator.Warning, CalendarWriteIndicators.Of(CalendarWriteStatus.Unconfirmed));
    }

    [Fact]
    public void The_two_failures_are_two_sentences()
    {
        // "Couldn't save this change" changed nothing on the server; "Couldn't confirm" may have
        // landed. Saying the gentler one for both reassures the user about a change never made.
        Assert.Equal(CalendarWriteLine.None, CalendarWriteIndicators.Line(CalendarWriteStatus.Idle));
        Assert.Equal(CalendarWriteLine.Saving, CalendarWriteIndicators.Line(CalendarWriteStatus.Saving));
        Assert.Equal(CalendarWriteLine.Saved, CalendarWriteIndicators.Line(CalendarWriteStatus.Saved));
        Assert.Equal(CalendarWriteLine.NotSaved, CalendarWriteIndicators.Line(CalendarWriteStatus.NotSaved));
        Assert.Equal(CalendarWriteLine.Unconfirmed, CalendarWriteIndicators.Line(CalendarWriteStatus.Unconfirmed));
    }

    [Fact]
    public void Every_status_the_core_has_is_mapped()
    {
        // A value added to the core's enum lands on the switch's discard; this names it.
        var mapped = new[]
        {
            CalendarWriteStatus.Idle, CalendarWriteStatus.Saving, CalendarWriteStatus.Saved,
            CalendarWriteStatus.NotSaved, CalendarWriteStatus.Unconfirmed,
        };
        Assert.Equal(mapped, Enum.GetValues<CalendarWriteStatus>());
    }

    [Fact]
    public void Only_the_warning_offers_a_retry()
    {
        // The retry is a refresh, and it only makes sense on a failure, offering it on a spinner or
        // a check would invite the user to "retry" a write that is fine.
        Assert.True(CalendarWriteIndicator.Warning.OffersRetry());
        Assert.False(CalendarWriteIndicator.Spinner.OffersRetry());
        Assert.False(CalendarWriteIndicator.Saved.OffersRetry());
        Assert.False(CalendarWriteIndicator.Hidden.OffersRetry());
    }
}
