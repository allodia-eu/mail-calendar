// An account's page and its links (docs/accounts.md rule 12): a use a link fills is a row naming
// the account it comes from and opening its page, never a switch; the accounts taking a use from it
// each say what they take; and Stop linking asks first, saying what is lost, and sends only on yes.
// The rows themselves are the core's (`account_page`), tested there.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AccountLinkedRulesTests
{
    private static readonly ControlState Free = new(true, false);
    private static readonly ControlState Running = new(false, true);
    private static readonly ControlState Held = new(false, false);

    private static readonly LinkedFromWords Words = new(
        AccountNamesTests.Words,
        new ByUses("takes calendar", "takes contacts", "takes both"),
        new ByUses("loses calendar", "loses contacts", "loses both"),
        "and has none of its own.",
        name => $"Stop linking {name}?");

    private static readonly LinkedAccount Nextcloud = new(
        "alice@example.org@cloud.example.org", "alice@example.org", new AccountDistinction.Kind(AccountKind.Dav));

    private static AccountUseSwitch Use(LinkedAccount? linked, ControlState control) =>
        new(AccountCapability.Calendar, linked is null, control, null, null, false, false, false, false, linked);

    private static AccountLinkedFrom Taking(
        AccountCapability[] uses, AccountCapability[]? leavesWithout = null, ControlState? stop = null) =>
        new(Nextcloud, uses, leavesWithout ?? [], stop ?? Free);

    [Fact]
    public void ALinkedUseIsARowNamingTheAccountItComesFromAndOpeningItsPage()
    {
        var row = AccountLinkedRules.Row(Use(Nextcloud, Free), AccountNamesTests.Words, name => $"From {name}");
        Assert.NotNull(row);
        Assert.Equal("From alice@example.org (DAV)", row.Subtitle);
        Assert.Equal(Nextcloud.Id, row.Opens);
        Assert.True(row.Enabled);
    }

    [Fact]
    public void AUseNoLinkFillsIsASwitch()
    {
        Assert.Null(AccountLinkedRules.Row(Use(null, Free), AccountNamesTests.Words, name => $"From {name}"));
    }

    [Fact]
    public void ALinkedUsesRowIsHeldWhileAChangeRunsOnThePage()
    {
        Assert.False(AccountLinkedRules.Row(Use(Nextcloud, Running), AccountNamesTests.Words, name => name)!.Enabled);
        Assert.False(AccountLinkedRules.Row(Use(Nextcloud, Held), AccountNamesTests.Words, name => name)!.Enabled);
    }

    [Fact]
    public void AnAccountTakingAUseIsNamedAndSaysWhichItTakes()
    {
        var both = AccountLinkedRules.Row("mailbox", Taking([AccountCapability.Calendar, AccountCapability.Contacts]), Words);
        Assert.Equal("alice@example.org (DAV)", both.Title);
        Assert.Equal("takes both", both.Line);
        Assert.Equal("Stop linking alice@example.org (DAV)?", both.ConfirmTitle);
        Assert.Equal("takes calendar", AccountLinkedRules.Row("mailbox", Taking([AccountCapability.Calendar]), Words).Line);
        Assert.Equal("takes contacts", AccountLinkedRules.Row("mailbox", Taking([AccountCapability.Contacts]), Words).Line);
    }

    [Fact]
    public void StoppingSaysWhatIsLostAndThatItHasNoneOfItsOwnOnlyWhenItHasNot()
    {
        Assert.Equal("loses calendar", AccountLinkedRules.StopMessage(Taking([AccountCapability.Calendar]), Words));
        Assert.Equal(
            "loses both and has none of its own.",
            AccountLinkedRules.StopMessage(
                Taking([AccountCapability.Contacts, AccountCapability.Calendar], [AccountCapability.Contacts]), Words));
        Assert.Equal(
            "loses contacts and has none of its own.",
            AccountLinkedRules.Row("mailbox", Taking([AccountCapability.Contacts], [AccountCapability.Contacts]), Words)
                .ConfirmMessage);
    }

    [Fact]
    public void StopLinkingIsSentOnYesOnlyAndNamesBothAccounts()
    {
        Assert.Null(AccountLinkedRules.StopAnswer(false, "mailbox", Taking([AccountCapability.Calendar])));
        var sent = Assert.IsType<AccountPageIntent.StopLinking>(
            AccountLinkedRules.StopAnswer(true, "mailbox", Taking([AccountCapability.Calendar])));
        Assert.Equal("mailbox", sent.Account);
        Assert.Equal(Nextcloud.Id, sent.From);
    }

    [Fact]
    public void StopLinkingIsTheCoresControlAndItsQuestionWaitsWhileAChangeRuns()
    {
        var running = AccountLinkedRules.Row("mailbox", Taking([AccountCapability.Calendar], stop: Running), Words);
        Assert.Equal(Running, running.Stop);
        Assert.False(running.ConfirmEnabled);
        var free = AccountLinkedRules.Row("mailbox", Taking([AccountCapability.Calendar]), Words);
        Assert.Equal(Free, free.Stop);
        Assert.True(free.ConfirmEnabled);
        // One question per account taking a use, kept open across a redraw by its key.
        var other = AccountLinkedRules.Row(
            "mailbox", new AccountLinkedFrom(new LinkedAccount("bob", "bob"), [AccountCapability.Calendar], [], Free), Words);
        Assert.NotEqual(free.Asking, other.Asking);
    }

    [Fact]
    public void ASeparateSignInSaysWhichUsesMoved()
    {
        var said = new ByUses("calendar moved", "contacts moved", "both moved");
        Assert.Equal("calendar moved", said.For([AccountCapability.Calendar]));
        Assert.Equal("contacts moved", said.For([AccountCapability.Contacts]));
        Assert.Equal("both moved", said.For([AccountCapability.Contacts, AccountCapability.Calendar]));
    }
}
