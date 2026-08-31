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
        Assert.Equal(CalendarWriteIndicator.Warning, CalendarWriteIndicators.Of(CalendarWriteStatus.Failed));
    }

    [Fact]
    public void Only_the_warning_offers_a_retry()
    {
        // The retry is a refresh, and it only makes sense on the unconfirmed state, offering it on a
        // spinner or a check would invite the user to "retry" a write that is fine.
        Assert.True(CalendarWriteIndicator.Warning.OffersRetry());
        Assert.False(CalendarWriteIndicator.Spinner.OffersRetry());
        Assert.False(CalendarWriteIndicator.Saved.OffersRetry());
        Assert.False(CalendarWriteIndicator.Hidden.OffersRetry());
    }
}
