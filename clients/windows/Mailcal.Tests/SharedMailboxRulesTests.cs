// What the Shared mailboxes section of an account's page draws for each answer the server gives
// (docs/shared-mailboxes.md rules 3, 7 and 8), and when the page asks the server again.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SharedMailboxRulesTests
{
    private static SharedMailboxEntry Mailbox(string? address, string name, bool selected, bool listed) =>
        new("f", name, address, selected, listed);

    private static SharedMailboxesView View(SharedMailboxSupport support, params SharedMailboxEntry[] mailboxes) =>
        new(support, mailboxes);

    [Fact]
    public void AMailboxIsNamedByItsAddressWithTheServersLabelBesideItWhenItDiffers()
    {
        Assert.Equal(
            new SharedRow("support@example.org", "Support Team", Withdrawn: false, Active: true),
            SharedMailboxRules.Row(Mailbox("support@example.org", "Support Team", true, true)));
        Assert.Null(SharedMailboxRules.Row(Mailbox("support@example.org", "support@example.org", false, true)).Label);
        Assert.Equal("team", SharedMailboxRules.Row(Mailbox(null, "team", false, true)).Title);
    }

    [Fact]
    public void AChosenMailboxTheServerStoppedListingSaysSo()
    {
        var row = SharedMailboxRules.Row(Mailbox("support@example.org", "Support", true, false));
        Assert.True(row.Withdrawn);
        Assert.True(row.Active, "it is kept until the person switches it off (rule 8)");
    }

    [Fact]
    public void TheLineUnderTheSwitchesSaysWhyThereIsNothingMoreToChoose()
    {
        Assert.Equal(SharedNote.NoneShared, SharedMailboxRules.Note(View(SharedMailboxSupport.Listed)));
        Assert.Equal(
            SharedNote.None,
            SharedMailboxRules.Note(View(SharedMailboxSupport.Listed, Mailbox(null, "team", false, true))));
        Assert.Equal(SharedNote.ByAddress, SharedMailboxRules.Note(View(SharedMailboxSupport.ByAddress)));
        Assert.Equal(SharedNote.NotConnected, SharedMailboxRules.Note(View(SharedMailboxSupport.NotConnected)));
        Assert.Equal(SharedNote.Unreachable, SharedMailboxRules.Note(View(SharedMailboxSupport.Unreachable)));
        Assert.Equal(SharedNote.None, SharedMailboxRules.Note(View(SharedMailboxSupport.Unsupported)));
    }

    [Fact]
    public void AnAccountThatCannotOpenASharedMailboxHasNoSection()
    {
        Assert.False(SharedMailboxRules.Shown(null), "nothing is drawn before the server answers");
        Assert.False(SharedMailboxRules.Shown(View(SharedMailboxSupport.Unsupported)));
        // One chosen while the account could open it is still there to switch off.
        Assert.True(SharedMailboxRules.Shown(View(SharedMailboxSupport.Unsupported, Mailbox(null, "team", true, false))));
        Assert.True(SharedMailboxRules.Shown(View(SharedMailboxSupport.Listed)));
        Assert.True(SharedMailboxRules.Shown(View(SharedMailboxSupport.Unreachable)));
    }

    [Fact]
    public void OnlyAMicrosoftAccountGetsTheAddressField()
    {
        foreach (var support in Enum.GetValues<SharedMailboxSupport>())
        {
            Assert.Equal(support == SharedMailboxSupport.ByAddress, SharedMailboxRules.TakesAddress(View(support)));
        }
    }

    [Fact]
    public void APageAsksAgainOnlyWhileTheAnswerMayStillChange()
    {
        Assert.True(SharedMailboxRules.MayHaveChanged(null, asking: false, usesChanged: false));
        Assert.True(SharedMailboxRules.MayHaveChanged(View(SharedMailboxSupport.NotConnected), asking: false, usesChanged: false));
        Assert.True(SharedMailboxRules.MayHaveChanged(View(SharedMailboxSupport.Unsupported), asking: false, usesChanged: false));
        // Listing again would be a request to the server on every signal.
        Assert.False(SharedMailboxRules.MayHaveChanged(View(SharedMailboxSupport.Listed), asking: false, usesChanged: false));
        Assert.False(SharedMailboxRules.MayHaveChanged(View(SharedMailboxSupport.Unreachable), asking: false, usesChanged: false));
        Assert.False(SharedMailboxRules.MayHaveChanged(null, asking: true, usesChanged: true), "one ask at a time");
    }

    [Fact]
    public void AListedAnswerIsAskedAgainOnceTheAccountsUsesChange()
    {
        // Switching mail, or the Microsoft use, off removes every chosen mailbox (rule 2), so a
        // listing that still shows one switched on is stale.
        Assert.True(SharedMailboxRules.MayHaveChanged(View(SharedMailboxSupport.Listed), asking: false, usesChanged: true));
        Assert.True(SharedMailboxRules.MayHaveChanged(View(SharedMailboxSupport.Unreachable), asking: false, usesChanged: true));

        var entry = AccountSettingsRulesTests.Entry(
            AccountKind.Microsoft,
            (AccountCapability.Mail, CapabilityState.On),
            (AccountCapability.SharedMailboxes, CapabilityState.On),
            (AccountCapability.Calendar, CapabilityState.Off));
        var before = SharedMailboxRules.Uses(entry);
        entry.Uses[1] = new AccountUse(AccountCapability.SharedMailboxes, CapabilityState.Off, null);
        var after = SharedMailboxRules.Uses(entry);
        Assert.NotEqual(before, after);
        // A use's problem is not a change of use.
        entry.Uses[2] = new AccountUse(AccountCapability.Calendar, CapabilityState.Off, UseProblem.Unreachable);
        Assert.Equal(after, SharedMailboxRules.Uses(entry));
    }

    [Fact]
    public void AnAnswerToAnOlderAskNeverReplacesAFresherOne()
    {
        Assert.True(SharedMailboxRules.Fresher(1, null));
        Assert.True(SharedMailboxRules.Fresher(3, 2));
        // The page's ask, started before a switch's, lands after the switch's answer was kept.
        Assert.False(SharedMailboxRules.Fresher(2, 3));
    }

    [Fact]
    public void TheSameAnswerTwiceIsTheSameAnswer()
    {
        // The generated records hold an array, which a record compares by reference.
        var listed = View(SharedMailboxSupport.Listed, Mailbox("a@example.org", "a", false, true));
        Assert.True(SharedMailboxRules.Same(listed, View(SharedMailboxSupport.Listed, Mailbox("a@example.org", "a", false, true))));
        Assert.False(SharedMailboxRules.Same(listed, View(SharedMailboxSupport.Listed, Mailbox("a@example.org", "a", true, true))));
        Assert.False(SharedMailboxRules.Same(listed, View(SharedMailboxSupport.Unreachable, Mailbox("a@example.org", "a", false, true))));
        Assert.False(SharedMailboxRules.Same(listed, null));
        Assert.True(SharedMailboxRules.Same(null, null));
    }
}
