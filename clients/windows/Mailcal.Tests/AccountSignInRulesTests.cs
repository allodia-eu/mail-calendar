// How a standards or JMAP account's page offers to change how it signs in (docs/accounts.md rule
// 17), what an expired sign-in's banner runs, and what a refused duplicate offers (rule 15).

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AccountSignInRulesTests
{
    private static AccountEntry Account(AccountKind kind, SignInMethod signIn) =>
        AccountSettingsRulesTests.Entry(kind, (AccountCapability.Mail, CapabilityState.On)) with { SignIn = signIn };

    private static AccountSignInOptions Options(bool signIn, bool password, string? label = null) =>
        new(signIn, label, password);

    [Fact]
    public void OnlyAStandardsOrJmapAccountHasTheSection()
    {
        foreach (var kind in new[] { AccountKind.Microsoft, AccountKind.Google, AccountKind.Dav })
        {
            Assert.False(AccountSettingsRules.SwitchesSignIn(kind));
            Assert.Null(AccountSettingsRules.SignIn(Account(kind, SignInMethod.Password), Options(true, true)));
        }
        Assert.True(AccountSettingsRules.SwitchesSignIn(AccountKind.Imap));
        Assert.True(AccountSettingsRules.SwitchesSignIn(AccountKind.Jmap));
    }

    [Fact]
    public void BeforeTheServerAnswersOnlyTheStatusIsDrawn()
    {
        var section = AccountSettingsRules.SignIn(Account(AccountKind.Imap, SignInMethod.Provider), null);
        Assert.Equal(new SignInSection(true, false, false, null, false), section);
        Assert.False(section!.OffersSwitch);

        section = AccountSettingsRules.SignIn(Account(AccountKind.Jmap, SignInMethod.Password), null);
        Assert.Equal(new SignInSection(false, false, false, null, false), section);
    }

    [Fact]
    public void ASignedInAccountSignsInAgainAndCanTakeAPassword()
    {
        var section = AccountSettingsRules.SignIn(
            Account(AccountKind.Imap, SignInMethod.Provider), Options(signIn: true, password: true, label: "Example"));
        // The label names the provider to switch to, and this account is there already.
        Assert.Equal(new SignInSection(true, true, false, null, true), section);
        Assert.True(section!.OffersSwitch);
    }

    [Fact]
    public void ASignedInAccountWhoseServerTakesNoPasswordOnlySignsInAgain()
    {
        var section = AccountSettingsRules.SignIn(
            Account(AccountKind.Imap, SignInMethod.Provider), Options(signIn: true, password: false));
        Assert.Equal(new SignInSection(true, true, false, null, false), section);
        // Signing in again is not a switch, so the line about what a switch keeps is not drawn.
        Assert.False(section!.OffersSwitch);
    }

    [Fact]
    public void APasswordAccountSwitchesToItsProviderByName()
    {
        var section = AccountSettingsRules.SignIn(
            Account(AccountKind.Jmap, SignInMethod.Password), Options(signIn: true, password: true, label: "Example"));
        // It has a password already, so nothing offers it one.
        Assert.Equal(new SignInSection(false, false, true, "Example", false), section);
        Assert.True(section!.OffersSwitch);

        section = AccountSettingsRules.SignIn(
            Account(AccountKind.Imap, SignInMethod.Password), Options(signIn: true, password: true));
        Assert.Null(section!.ProviderLabel);
    }

    [Fact]
    public void AServerThatOffersNoSignInOffersAPasswordAccountNothing()
    {
        var section = AccountSettingsRules.SignIn(
            Account(AccountKind.Imap, SignInMethod.Password), Options(signIn: false, password: true));
        Assert.Equal(new SignInSection(false, false, false, null, false), section);
    }

    [Fact]
    public void AnExpiredSignInIsRemediedByTheFlowItsProviderNeeds()
    {
        Assert.Equal(SignInRemedy.ProviderConsent, AccountSettingsRules.Remedy(AccountProvider.Microsoft));
        Assert.Equal(SignInRemedy.ProviderConsent, AccountSettingsRules.Remedy(AccountProvider.Google));
        Assert.Equal(SignInRemedy.JmapReauth, AccountSettingsRules.Remedy(AccountProvider.JmapOauth));
        // A standards account signed in through its provider has no password to ask for.
        Assert.Equal(SignInRemedy.AccountSignIn, AccountSettingsRules.Remedy(AccountProvider.ImapOauth));
        Assert.Equal(SignInRemedy.Settings, AccountSettingsRules.Remedy(AccountProvider.Password));
        Assert.Equal(SignInRemedy.Settings, AccountSettingsRules.Remedy(AccountProvider.Jmap));
        Assert.Equal(SignInRemedy.Settings, AccountSettingsRules.Remedy(null));
    }

    [Fact]
    public void ARefusedDuplicateOffersTheConfigToTheAccountAlreadyThere()
    {
        var twice = new MailcalException.AlreadySetUp("alice@example.org@imap.example.org");
        var offer = AccountSettingsRules.UpdateOffer(twice, "[imap]\n");
        Assert.Equal("alice@example.org@imap.example.org", offer?.AccountId);
        Assert.Equal("[imap]\n", offer?.ConfigToml);
        Assert.Empty(offer!.Unreached);
        // What the sign-in said it would not reach goes with the offer, which asks about it.
        var unreached = AccountSettingsRules.UpdateOffer(
            twice, "[imap]\n", [AccountCapability.Contacts, AccountCapability.Mail, AccountCapability.Calendar]);
        Assert.Equal([AccountCapability.Calendar, AccountCapability.Contacts], unreached!.Unreached);
        // Any other refusal is said and offers nothing.
        Assert.Null(AccountSettingsRules.UpdateOffer(new MailcalException.Connect("no route to host"), "[imap]\n"));
        Assert.Null(AccountSettingsRules.UpdateOffer(new MailcalException.SignInRefused("JMAP HTTP 401"), "[imap]\n"));
        Assert.Null(AccountSettingsRules.UpdateOffer(twice, string.Empty));
    }

    private static AccountEntry Calendar(bool nextcloud, SignInMethod signIn) =>
        AccountSettingsRulesTests.Entry(AccountKind.Dav, (AccountCapability.Calendar, CapabilityState.On))
            with { SignIn = signIn, Nextcloud = nextcloud };

    private static AccountSignInOptions NextcloudOffered(bool offered) => new(false, null, false, offered);

    [Fact]
    public void AnAccountANextcloudSignInMadeSignsInWithNextcloudAgainAndNeverSwitchesToAPassword()
    {
        // It reports Provider, which would otherwise read as a mailbox's provider sign-in.
        var entry = Calendar(nextcloud: true, SignInMethod.Provider);
        Assert.True(AccountSettingsRules.MayHaveSignInSection(entry));
        Assert.True(AccountSettingsRules.SignInDrawnBeforeAnswer(entry));
        Assert.Equal(
            new SignInSection(true, false, false, null, false, Nextcloud: true),
            AccountSettingsRules.SignIn(entry, null));
        var section = AccountSettingsRules.SignIn(entry, NextcloudOffered(true) with { SignIn = true, Password = true });
        Assert.Equal(new SignInSection(true, true, false, null, false, Nextcloud: true), section);
        Assert.False(section!.OffersSwitch);
    }

    [Fact]
    public void ATypedPasswordCalendarAccountAtNextcloudSwitchesToNextcloudByName()
    {
        var entry = Calendar(nextcloud: false, SignInMethod.Password);
        Assert.True(AccountSettingsRules.MayHaveSignInSection(entry));
        // Drawn only once its server has said it is Nextcloud.
        Assert.False(AccountSettingsRules.SignInDrawnBeforeAnswer(entry));
        Assert.Null(AccountSettingsRules.SignIn(entry, null));
        Assert.Null(AccountSettingsRules.SignIn(entry, NextcloudOffered(false)));
        var section = AccountSettingsRules.SignIn(entry, NextcloudOffered(true));
        Assert.Equal(new SignInSection(false, false, true, "Nextcloud", false, Nextcloud: true), section);
        // A calendar and contacts account has no mail or links for that line to keep.
        Assert.False(section!.OffersSwitch);
    }

    [Fact]
    public void AnExpiredNextcloudSignInIsSignedInWithNextcloudWhateverProviderItReports()
    {
        foreach (var provider in new AccountProvider?[] { AccountProvider.Password, AccountProvider.ImapOauth, null })
        {
            Assert.Equal(SignInRemedy.Nextcloud, AccountSettingsRules.Remedy(provider, nextcloud: true));
        }
    }

    [Fact]
    public void ARefusedNextcloudSignInOffersItsGrantAndNeverPrintsIt()
    {
        var offer = new ExistingAccountOffer("alice@cloud.example.org", string.Empty, [], NextcloudGrant: "secret");
        Assert.DoesNotContain("secret", offer.ToString());
        Assert.DoesNotContain("[imap]", new ExistingAccountOffer("a", "[imap]\npassword = \"x\"", []).ToString());
    }

    [Fact]
    public void SwitchingHowAnAccountSignsInRedrawsItsPage()
    {
        var password = Account(AccountKind.Imap, SignInMethod.Password);
        var drawn = AccountSettingsRules.Fingerprint(new AccountsSnapshot([password]), []);
        Assert.NotEqual(
            drawn,
            AccountSettingsRules.Fingerprint(new AccountsSnapshot([password with { SignIn = SignInMethod.Provider }]), []));
        var calendar = Calendar(nextcloud: false, SignInMethod.Password);
        Assert.NotEqual(
            AccountSettingsRules.Fingerprint(new AccountsSnapshot([calendar]), []),
            AccountSettingsRules.Fingerprint(new AccountsSnapshot([calendar with { Nextcloud = true }]), []));
    }
}
