// What a refused duplicate offers (docs/accounts.md rule 15), and that switching how an account
// signs in redraws its page. Which ways in the page offers, and what an expired sign-in's banner
// runs, are the core's, pinned in mailcal-bindings' account_page_sign_in_tests.rs and
// app_account_page_browser_tests.rs.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AccountSignInRulesTests
{
    private static AccountEntry Account(AccountKind kind, SignInMethod signIn) =>
        AccountSettingsRulesTests.Entry(kind, (AccountCapability.Mail, CapabilityState.On)) with { SignIn = signIn };

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
        var drawn = AccountSettingsRules.Fingerprint(new AccountsSnapshot([password]));
        Assert.NotEqual(
            drawn,
            AccountSettingsRules.Fingerprint(new AccountsSnapshot([password with { SignIn = SignInMethod.Provider }])));
        var calendar = Calendar(nextcloud: false, SignInMethod.Password);
        Assert.NotEqual(
            AccountSettingsRules.Fingerprint(new AccountsSnapshot([calendar])),
            AccountSettingsRules.Fingerprint(new AccountsSnapshot([calendar with { Nextcloud = true }])));
    }
}
