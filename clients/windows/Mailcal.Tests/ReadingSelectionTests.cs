// Which row the list highlights for the message in the reading pane. The twin of
// ReadingAdvanceTests: together they pin the whole of "archive, and the next message opens, and
// the list shows you which one".
//
// The case that was actually reported is `TheRowTheReadingPaneAdvancedToIsTheOneHighlighted`: the
// pane advanced but the highlight stayed on the archived row, because on Windows the highlight was
// only ever assigned by a click handler and nothing had clicked.

using System.Collections.Generic;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ReadingSelectionTests
{
    private static RowStop Flat(string key, string account = "a0") =>
        new(account, key, new List<string> { key });

    private static RowStop Thread(string latest, IEnumerable<string> members, string account = "a0") =>
        new(account, latest, new List<string>(members));

    [Fact]
    public void TheOpenMessagesOwnRowIsTheHighlightedOne()
    {
        var rows = new List<RowStop> { Flat("1"), Flat("2"), Flat("3") };
        Assert.Equal(1, ReadingSelection.RowOf("a0", "2", rows));
    }

    // The reported bug: archiving row 2 advances the pane to row 3, and the list must move the
    // highlight with it, nothing clicked row 3.
    [Fact]
    public void TheRowTheReadingPaneAdvancedToIsTheOneHighlighted()
    {
        var before = new List<RowStop> { Flat("1"), Flat("2"), Flat("3") };
        var face = AvatarFixture.Item();
        var next = ReadingAdvance.Next(
            new MessageStop("a0", "2", "s", "f", face, "d"),
            new List<MessageStop>
            {
                new("a0", "1", "s", "f", face, "d"),
                new("a0", "2", "s", "f", face, "d"),
                new("a0", "3", "s", "f", face, "d"),
            });
        Assert.Equal("3", next?.Key);

        // ...and once the archived row leaves the list, the advanced-to message is still matched.
        var after = new List<RowStop> { Flat("1"), Flat("3") };
        Assert.Equal(1, ReadingSelection.RowOf("a0", next!.Value.Key, after));
        Assert.Equal(2, ReadingSelection.RowOf("a0", next!.Value.Key, before));
    }

    [Fact]
    public void AMessageNoLongerInTheListHighlightsNothing() =>
        Assert.Null(ReadingSelection.RowOf("a0", "9", new List<RowStop> { Flat("1"), Flat("2") }));

    [Fact]
    public void AnEmptyListHighlightsNothing() =>
        Assert.Null(ReadingSelection.RowOf("a0", "1", new List<RowStop>()));

    // A conversation row stands for every message on it: opening a sub-row of an expanded thread
    // highlights the conversation, not nothing.
    [Fact]
    public void AConversationIsHighlightedForAnyMessageOnIt()
    {
        var rows = new List<RowStop> { Flat("1"), Thread("t-latest", new[] { "t-latest", "t-old" }) };
        Assert.Equal(1, ReadingSelection.RowOf("a0", "t-latest", rows));
        Assert.Equal(1, ReadingSelection.RowOf("a0", "t-old", rows));
    }

    // A provider key is unique only within its account, so two accounts can mint the same one,
    // matching on the key alone would highlight a row in the wrong mailbox.
    [Fact]
    public void TheSameKeyInAnotherAccountIsADifferentRow()
    {
        var rows = new List<RowStop> { Flat("1", "a0"), Flat("1", "a1") };
        Assert.Equal(1, ReadingSelection.RowOf("a1", "1", rows));
        Assert.Equal(0, ReadingSelection.RowOf("a0", "1", rows));
    }
}
