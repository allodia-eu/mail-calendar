// Leaving a composer and discarding one (docs/drafts.md, "Leaving a composer"), pinned. Both fail
// silently in the running app: a leave that saved an untouched reply puts a quote-only message in
// Drafts, and a Discard that did not ask throws away a draft the user had kept.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ComposerExitTests
{
    [Fact]
    public void LeavingAComposerSomethingWasWrittenInSavesIt() =>
        Assert.Equal(ComposerLeave.SaveAndClose, ComposerExit.OnLeave(keepsDraft: true, written: true));

    // A reply holding only its quote, and a resumed draft only looked at, are not saved again: the
    // first would put a quote-only message in Drafts, and the second is there already.
    [Fact]
    public void LeavingAnUntouchedComposerOnlyClosesIt() =>
        Assert.Equal(ComposerLeave.Close, ComposerExit.OnLeave(keepsDraft: true, written: false));

    [Fact]
    public void AComposerKeepingNoDraftIsNeverSavedOnTheWayOut() =>
        Assert.Equal(ComposerLeave.Close, ComposerExit.OnLeave(keepsDraft: false, written: true));

    // Words written, a copy in Drafts, or both: each is something a Discard takes away.
    [Theory]
    [InlineData(true, false)]
    [InlineData(false, true)]
    [InlineData(true, true)]
    public void DiscardAsksWhenSomethingWouldBeLost(bool written, bool stored) =>
        Assert.True(ComposerExit.AsksBeforeDiscard(written, stored));

    [Fact]
    public void DiscardingAnEmptyComposerWithNothingStoredAsksNothing() =>
        Assert.False(ComposerExit.AsksBeforeDiscard(written: false, stored: false));
}
