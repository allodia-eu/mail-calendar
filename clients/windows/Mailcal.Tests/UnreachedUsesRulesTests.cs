// A calendar and contacts the account's sign-in does not reach (docs/accounts.md rule 18): which
// title a set of uses gets, whether a switch asks and what each answer passes, and the form setup
// and the account's page draw from the core's section.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class UnreachedUsesRulesTests
{
    private static readonly ControlState Free = new(true, false);

    /// <summary>The core's section for <paramref name="uses"/>, nothing running.</summary>
    internal static AccountSeparateServer Section(params AccountCapability[] uses) =>
        new(uses, "dav.example.org:8443", "alice", null, Free, Free, null);

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
    public void TheFormIsTheSectionTheCoreDrawsForCalendarOrContacts()
    {
        Assert.Null(UnreachedUsesRules.Step("alice@imap", null));
        Assert.Null(UnreachedUsesRules.Step("alice@imap", Section(AccountCapability.Mail)));

        var step = UnreachedUsesRules.Step("alice@imap", Section(AccountCapability.Calendar))!;
        Assert.Equal("alice@imap", step.AccountId);
        Assert.Equal([AccountCapability.Calendar], step.Uses);
        Assert.Equal(UnreachedTitle.Calendar, step.Title);
        Assert.Equal("dav.example.org:8443", step.Host);
        Assert.Equal("alice", step.Username);
        Assert.Equal(
            UnreachedTitle.Both,
            UnreachedUsesRules.Step("alice@imap", Section(AccountCapability.Calendar, AccountCapability.Contacts))!.Title);
    }

    [Fact]
    public void TheOfferIsTheSectionsAndARedrawFollowsWhatChanged()
    {
        var plain = UnreachedUsesRules.Step("alice@imap", Section(AccountCapability.Calendar))!;
        Assert.Equal(SeparateSignInOffer.None, SeparateServerSignIn.Offer(plain));
        var nextcloud = plain with { Section = plain.Section with { SignIn = AccountSeparateSignIn.Nextcloud } };
        Assert.Equal(SeparateSignInOffer.Nextcloud, SeparateServerSignIn.Offer(nextcloud));
        Assert.True(SeparateServerSignIn.Offers(nextcloud));

        // A read that changed nothing draws nothing; a connect starting or failing does.
        Assert.Equal(
            UnreachedUsesRules.Fingerprint(Section(AccountCapability.Calendar)),
            UnreachedUsesRules.Fingerprint(Section(AccountCapability.Calendar)));
        var running = Section(AccountCapability.Calendar) with { Connect = new ControlState(false, true) };
        Assert.NotEqual(UnreachedUsesRules.Fingerprint(plain.Section), UnreachedUsesRules.Fingerprint(running));
        var refused = Section(AccountCapability.Calendar) with { Failed = new AccountChangeRefusal.SignInRefused() };
        Assert.NotEqual(UnreachedUsesRules.Fingerprint(plain.Section), UnreachedUsesRules.Fingerprint(refused));
    }
}
