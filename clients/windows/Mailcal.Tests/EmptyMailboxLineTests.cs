// What the list says when it has no rows (docs/folder-pane.md, rule 20).
//
// The rule is two branches, a number and one decision about whether to offer a setting, which is
// exactly the kind that regresses without anyone noticing: "No mail from the last 0 months" over
// an account that syncs everything is a confident, wrong claim, and offering to widen a depth that
// is already all-time promises mail that does not exist. Neither shows up in a screenshot, and the
// WinUI half cannot be reached from this assembly.
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class EmptyMailboxLineTests
{
    private const string NoMail = "No mail in this folder yet.";

    private static string Line(EmptyReason? reason) =>
        EmptyMailboxLine.For(reason, NoMail, months => $"No mail from the last {months} months.");

    [Fact]
    public void ADepthBoundedListNamesItsMonthCount()
    {
        Assert.Equal("No mail from the last 3 months.", Line(new EmptyReason.OutsideSyncDepth(3)));
        Assert.Equal("No mail from the last 24 months.", Line(new EmptyReason.OutsideSyncDepth(24)));
    }

    [Fact]
    public void AFolderHeldWholeSaysItIsEmptyInsteadOfCountingMonths()
    {
        // The core sends NoMail, never OutsideSyncDepth(0), but the sentence has to come from the
        // branch and not from the number, or a future zero would read as "the last 0 months".
        Assert.Equal(NoMail, Line(new EmptyReason.NoMail()));
    }

    [Fact]
    public void AListWithRowsStatesNothingAtAll()
    {
        // The panel keys its visibility off the same null the core sends for every list that has
        // rows, so an empty string here and a hidden panel there are one decision.
        Assert.Equal(string.Empty, Line(null));
    }

    [Fact]
    public void OnlyADepthBoundedListOffersTheSetting()
    {
        // Widening finds nothing for a folder this device already holds whole, so the button is
        // not merely useless there: it claims mail exists that does not.
        Assert.True(EmptyMailboxLine.OffersSyncDepth(new EmptyReason.OutsideSyncDepth(6)));
        Assert.False(EmptyMailboxLine.OffersSyncDepth(new EmptyReason.NoMail()));
        Assert.False(EmptyMailboxLine.OffersSyncDepth(null));
    }
}
