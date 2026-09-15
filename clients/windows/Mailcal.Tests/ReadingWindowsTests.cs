// The two rules behind opening a message in a window of its own (docs/reading-window.md).
//
// Both are invisible in the running app when wrong. A reader id that is not derived from the
// message stacks a second window on every double-click instead of raising the open one, and the
// two then hold two copies of one body. A double-click the list reads as two single clicks opens
// no window at all, while two deliberate single clicks read as a double-click open one nobody
// asked for.

using System;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ReadingWindowIdTests
{
    [Fact]
    public void OneMessageAlwaysNamesOneWindow()
    {
        // What makes a second double-click on a row reach the window already up: the id is derived
        // from the message, never minted per click.
        Assert.Equal(
            ReadingWindows.IdFor("acct-1", "m1"),
            ReadingWindows.IdFor("acct-1", "m1"));
    }

    [Fact]
    public void TwoAccountsSharingAKeyAreTwoWindows()
    {
        // A provider key is unique within its account and not beyond it, so the account has to be
        // part of the id: without it, the same key in two accounts would share one window and the
        // second open would silently show the first account's message.
        Assert.NotEqual(
            ReadingWindows.IdFor("acct-1", "shared-key"),
            ReadingWindows.IdFor("acct-2", "shared-key"));
    }

    [Fact]
    public void TwoMessagesInOneAccountAreTwoWindows()
    {
        Assert.NotEqual(
            ReadingWindows.IdFor("acct-1", "m1"),
            ReadingWindows.IdFor("acct-1", "m2"));
    }
}

public class RowDoubleClickTests
{
    private static readonly TimeSpan Interval = TimeSpan.FromMilliseconds(500);
    private static readonly DateTimeOffset T0 = new(2026, 9, 15, 12, 0, 0, TimeSpan.Zero);

    [Fact]
    public void TheFirstClickIsNeverADoubleClick()
    {
        var clicks = new RowDoubleClick(Interval);
        Assert.False(clicks.CompletesDoubleClick("row-1", T0));
    }

    [Fact]
    public void TwoQuickClicksOnOneRowAreADoubleClick()
    {
        var clicks = new RowDoubleClick(Interval);
        Assert.False(clicks.CompletesDoubleClick("row-1", T0));
        Assert.True(clicks.CompletesDoubleClick("row-1", T0.AddMilliseconds(120)));
    }

    [Fact]
    public void AClickExactlyOnTheIntervalStillCounts()
    {
        // The boundary is inclusive, so a desktop set to 500ms honours a 500ms double-click rather
        // than dropping the one click in a hundred that lands exactly on it.
        var clicks = new RowDoubleClick(Interval);
        Assert.False(clicks.CompletesDoubleClick("row-1", T0));
        Assert.True(clicks.CompletesDoubleClick("row-1", T0 + Interval));
    }

    [Fact]
    public void TwoSlowClicksAreTwoSingleClicks()
    {
        var clicks = new RowDoubleClick(Interval);
        Assert.False(clicks.CompletesDoubleClick("row-1", T0));
        Assert.False(clicks.CompletesDoubleClick("row-1", T0.AddMilliseconds(501)));
    }

    [Fact]
    public void TwoQuickClicksOnDifferentRowsAreNotADoubleClick()
    {
        // Running down a list quickly is not a double-click, and reading it as one would open a
        // window on a message the user only glanced at.
        var clicks = new RowDoubleClick(Interval);
        Assert.False(clicks.CompletesDoubleClick("row-1", T0));
        Assert.False(clicks.CompletesDoubleClick("row-2", T0.AddMilliseconds(80)));
    }

    [Fact]
    public void ASecondClickOnANewRowStartsCountingThatRow()
    {
        var clicks = new RowDoubleClick(Interval);
        Assert.False(clicks.CompletesDoubleClick("row-1", T0));
        Assert.False(clicks.CompletesDoubleClick("row-2", T0.AddMilliseconds(80)));
        Assert.True(clicks.CompletesDoubleClick("row-2", T0.AddMilliseconds(160)));
    }

    [Fact]
    public void ATripleClickOpensOneWindowRatherThanTwo()
    {
        // A completed double-click forgets what it was counting, so the third click starts again
        // instead of completing a second double-click with the second.
        var clicks = new RowDoubleClick(Interval);
        Assert.False(clicks.CompletesDoubleClick("row-1", T0));
        Assert.True(clicks.CompletesDoubleClick("row-1", T0.AddMilliseconds(100)));
        Assert.False(clicks.CompletesDoubleClick("row-1", T0.AddMilliseconds(200)));
    }

    [Fact]
    public void TheTrailingClickOfADoubleClickIsRefused()
    {
        // THE ORDERING THIS EXISTS FOR, measured on the running app. A double-click raises
        // ItemClick, then DoubleTapped, then ItemClick AGAIN about 80ms later. The first click
        // opens the row in the reading pane and records what the pane held; the double-tap opens
        // the window and puts the pane back. If that trailing click were taken for a fresh first
        // click it would re-open the row in the pane and undo the correction, which is exactly the
        // bug this was written after.
        var clicks = new RowDoubleClick(Interval);
        Assert.False(clicks.CompletesDoubleClick("row-1", T0), "the first press opens the pane");
        Assert.True(
            clicks.CompletesDoubleClick("row-1", T0.AddMilliseconds(80)),
            "the trailing press is refused, so the pane keeps what the double-tap restored");
    }
}
