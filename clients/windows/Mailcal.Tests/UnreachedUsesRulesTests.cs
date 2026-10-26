// A calendar and contacts the account's sign-in does not reach (docs/accounts.md rule 18): which
// title a set of uses gets, whether a switch asks and what each answer passes, and the form setup
// and the account's page offer from the core's AccountEntry.SeparateServer.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class UnreachedUsesRulesTests
{
    private static AccountEntry Mailbox(SeparateServer? separate) => AccountSettingsRulesTests.Entry(
        AccountKind.Imap,
        (AccountCapability.Mail, CapabilityState.On),
        (AccountCapability.Calendar, CapabilityState.On),
        (AccountCapability.Contacts, CapabilityState.On)) with { SeparateServer = separate };

    [Fact]
    public void ATitleNamesWhatTheSetHolds()
    {
        Assert.Equal(UnreachedTitle.Both, UnreachedUsesRules.Title([AccountCapability.Contacts, AccountCapability.Calendar]));
        Assert.Equal(UnreachedTitle.Calendar, UnreachedUsesRules.Title([AccountCapability.Calendar]));
        Assert.Equal(UnreachedTitle.Contacts, UnreachedUsesRules.Title([AccountCapability.Contacts, AccountCapability.Mail]));
        // Mail and colleagues are never on a server of their own.
        Assert.Null(UnreachedUsesRules.Title([AccountCapability.Mail, AccountCapability.Colleagues]));
        Assert.Null(UnreachedUsesRules.Title([]));
        Assert.Null(UnreachedUsesRules.Title(null));
    }

    [Fact]
    public void ASwitchAsksOnlyWhenItWouldLoseSomething()
    {
        Assert.True(UnreachedUsesRules.Asks([AccountCapability.Calendar]));
        Assert.True(UnreachedUsesRules.Asks([AccountCapability.Contacts]));
        // A start that names nothing (or a record built without the field) behaves as before.
        Assert.False(UnreachedUsesRules.Asks([]));
        Assert.False(UnreachedUsesRules.Asks(null));
        Assert.False(UnreachedUsesRules.Asks(new AccountSignInStart("https://auth.example.org", "pending").Unreached));
    }

    [Fact]
    public void EachAnswerPassesItsChoiceAndCancelPassesNothing()
    {
        Assert.Equal(UnreachedUses.KeepOnPassword, UnreachedUsesRules.Choice(UnreachedAnswer.Keep));
        Assert.Equal(UnreachedUses.StopUsing, UnreachedUsesRules.Choice(UnreachedAnswer.Stop));
        Assert.Null(UnreachedUsesRules.Choice(UnreachedAnswer.Cancel));
    }

    [Fact]
    public void ThePageOffersTheFormForExactlyTheUsesTheCoreNames()
    {
        Assert.Null(UnreachedUsesRules.Step(Mailbox(null)));

        var entry = Mailbox(new SeparateServer([AccountCapability.Calendar], "dav.example.org:8443", "alice"));
        var step = UnreachedUsesRules.Step(entry)!;
        Assert.Equal(entry.Id, step.AccountId);
        Assert.Equal([AccountCapability.Calendar], step.Uses);
        Assert.Equal(UnreachedTitle.Calendar, step.Title);
        Assert.Equal("dav.example.org:8443", step.Host);
        Assert.Equal("alice", step.Username);
        Assert.True(UnreachedUsesRules.Offers(step, AccountCapability.Calendar));
        Assert.False(UnreachedUsesRules.Offers(step, AccountCapability.Contacts));
        Assert.False(UnreachedUsesRules.Offers(null, AccountCapability.Calendar));

        // Both, calendar first whatever order they came in, and the address when no login is held.
        var both = UnreachedUsesRules.Step(Mailbox(new SeparateServer(
            [AccountCapability.Contacts, AccountCapability.Calendar], "dav.example.org", " ")))!;
        Assert.Equal([AccountCapability.Calendar, AccountCapability.Contacts], both.Uses);
        Assert.Equal(UnreachedTitle.Both, both.Title);
        Assert.Equal("alice@example.org", both.Username);
    }

    [Fact]
    public void SetupShowsTheStepOnlyForANewAccountTheCoreNamesAServerFor()
    {
        var entry = Mailbox(new SeparateServer([AccountCapability.Contacts], "dav.example.org", "alice@example.org"));
        var snapshot = new AccountsSnapshot([entry]);
        var step = UnreachedUsesRules.SetupStep(snapshot, entry.Id)!;
        Assert.Equal(UnreachedTitle.Contacts, step.Title);
        // Skip switches off exactly these, and Connect moves exactly these.
        Assert.Equal([AccountCapability.Contacts], step.Uses);

        Assert.Null(UnreachedUsesRules.SetupStep(new AccountsSnapshot([Mailbox(null)]), entry.Id));
        Assert.Null(UnreachedUsesRules.SetupStep(snapshot, "someone-else"));
    }
}
