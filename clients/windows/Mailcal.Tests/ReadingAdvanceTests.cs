// The archive/delete auto-advance rule: next one down, else the one above, else nothing, the twin
// of the Apple client's AutoAdvanceTests. Pinning it here rather than through the UI is the point of
// keeping ReadingAdvance pure; the end-of-list case in particular is the one a hand test skips,
// because it only shows up on the last message of a folder.

using System.Collections.Generic;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ReadingAdvanceTests
{
    private static MessageStop Stop(string key, string account = "a0") =>
        new(account, key, $"s-{key}", $"f-{key}", AvatarFixture.Item(), "d");

    [Fact]
    public void AdvancesToTheNextMessageDown()
    {
        var stops = new List<MessageStop> { Stop("1"), Stop("2"), Stop("3") };
        Assert.Equal("3", ReadingAdvance.Next(Stop("2"), stops)?.Key);
    }

    [Fact]
    public void AdvancesFromTheFirstMessage()
    {
        var stops = new List<MessageStop> { Stop("1"), Stop("2"), Stop("3") };
        Assert.Equal("2", ReadingAdvance.Next(Stop("1"), stops)?.Key);
    }

    // The end of the list falls *back* rather than emptying the pane.
    [Fact]
    public void TheLastMessageFallsBackToTheOneAbove()
    {
        var stops = new List<MessageStop> { Stop("1"), Stop("2"), Stop("3") };
        Assert.Equal("2", ReadingAdvance.Next(Stop("3"), stops)?.Key);
    }

    [Fact]
    public void TheOnlyMessageLeavesNothingToOpen() =>
        Assert.Null(ReadingAdvance.Next(Stop("1"), new List<MessageStop> { Stop("1") }));

    [Fact]
    public void AnEmptyListLeavesNothingToOpen() =>
        Assert.Null(ReadingAdvance.Next(Stop("1"), new List<MessageStop>()));

    // A message no longer on screen has no neighbours: the pane empties, as it did before.
    [Fact]
    public void AMessageNotInTheListLeavesNothingToOpen()
    {
        var stops = new List<MessageStop> { Stop("1"), Stop("2") };
        Assert.Null(ReadingAdvance.Next(Stop("9"), stops));
    }

    // A provider key is unique only within its account, so two accounts can mint the same one.
    [Fact]
    public void TheSameKeyInAnotherAccountIsADifferentMessage()
    {
        var stops = new List<MessageStop> { Stop("1", "a0"), Stop("1", "a1"), Stop("2", "a1") };
        var next = ReadingAdvance.Next(Stop("1", "a1"), stops);
        Assert.Equal("a1", next?.Account);
        Assert.Equal("2", next?.Key);
    }

    // The chosen stop carries the header the reading view will show, not just an identity.
    [Fact]
    public void TheChosenStopCarriesItsReadingHeader()
    {
        var stops = new List<MessageStop> { Stop("1"), Stop("2") };
        var next = ReadingAdvance.Next(Stop("1"), stops);
        Assert.Equal("s-2", next?.Subject);
        Assert.Equal("f-2", next?.From);
    }
}
