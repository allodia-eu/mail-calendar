// What Settings → Accounts' list says about an account, from a snapshot entry (docs/accounts.md
// rules 11 to 13), and when its page is drawn again. The page's uses, links and removal are the
// core's (`account_page`), tested there.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AccountSettingsRulesTests
{
    internal static LinkedAccount Linked(string id) => new(id, id);

    internal static AccountEntry Entry(AccountKind kind, params (AccountCapability, CapabilityState)[] uses) => new(
        Id: "alice@example.org@imap.example.org",
        Address: "alice@example.org",
        Kind: kind,
        SignIn: SignInMethod.Password,
        Uses: uses.Select(use => new AccountUse(use.Item1, use.Item2, null)).ToArray(),
        Links: new AccountLinksView(null, null, null),
        LinkedFrom: [],
        LinkCandidates: new LinkCandidates([], [], [], []),
        Endpoints: null);

    internal static AccountEntry MailboxOnly() => Entry(
        AccountKind.Imap,
        (AccountCapability.Mail, CapabilityState.On),
        (AccountCapability.Calendar, CapabilityState.Off),
        (AccountCapability.Contacts, CapabilityState.Off));

    [Fact]
    public void AUseWaitingOnPermissionIsOneTheAccountIsUsedForAndOneTheServerLacksIsNot()
    {
        var entry = Entry(
            AccountKind.Google,
            (AccountCapability.Mail, CapabilityState.On),
            (AccountCapability.Calendar, CapabilityState.NeedsPermission),
            (AccountCapability.Contacts, CapabilityState.NotOffered));
        Assert.Equal([AccountCapability.Mail, AccountCapability.Calendar], AccountSettingsRules.InUse(entry));
        Assert.True(AccountSettingsRules.NeedsPermission(entry));
        Assert.Equal([AccountCapability.Calendar], AccountSettingsRules.Withheld(entry));
    }

    [Fact]
    public void TheRowNamesTheUsesInOrderAndTheLinks()
    {
        var entry = MailboxOnly() with
        {
            Links = new AccountLinksView(Linked("alice@cloud.example"), null, null),
        };
        Assert.Equal([AccountCapability.Mail], AccountSettingsRules.InUse(entry));
        Assert.Equal([(AccountCapability.Calendar, Linked("alice@cloud.example"))], AccountSettingsRules.Linked(entry));
    }

    [Fact]
    public void RemovingNamesTheAccountsThatLoseTheirLink()
    {
        var entry = MailboxOnly();
        Assert.Null(AccountSettingsRules.Unlinked(entry, AccountNamesTests.Words));
        entry = entry with
        {
            LinkedFrom = [Linked("a@example.org"), Linked("b@example.org") with { Distinction = new AccountDistinction.Kind(AccountKind.Dav) }],
        };
        Assert.Equal("a@example.org, b@example.org (DAV)", AccountSettingsRules.Unlinked(entry, AccountNamesTests.Words));
    }

    [Fact]
    public void ASignalThatChangedNothingOnThePageLeavesItAlone()
    {
        var snapshot = new AccountsSnapshot([MailboxOnly()]);
        var drawn = AccountSettingsRules.Fingerprint(snapshot);
        Assert.Equal(drawn, AccountSettingsRules.Fingerprint(new AccountsSnapshot([MailboxOnly()])));

        // A suggestion arriving and a link each change what the page shows.
        var suggested = MailboxOnly() with
        {
            LinkCandidates = new LinkCandidates([Linked("a@cloud.example")], [], [], ["a@cloud.example"]),
        };
        Assert.NotEqual(drawn, AccountSettingsRules.Fingerprint(new AccountsSnapshot([suggested])));
        // So does a mailbox listed in the mail settings after the page was drawn, which is where
        // its Mail section comes from.
        Assert.NotEqual(drawn, AccountSettingsRules.Fingerprint(snapshot, ["mail:" + MailboxOnly().Id]));
        // And an account of the same address arriving, which changes how this one is named.
        var named = MailboxOnly() with { Distinction = new AccountDistinction.Server("imap.example.org") };
        Assert.NotEqual(drawn, AccountSettingsRules.Fingerprint(new AccountsSnapshot([named])));
    }

    [Fact]
    public void AUseThatStopsOrStartsConnectingRedrawsThePage()
    {
        var connected = Entry(
            AccountKind.Imap,
            (AccountCapability.Mail, CapabilityState.On),
            (AccountCapability.Calendar, CapabilityState.On));
        var drawn = AccountSettingsRules.Fingerprint(new AccountsSnapshot([connected]));

        // The calendar's dial ends after the page was drawn, and Refresh can clear it again.
        var failed = connected with { Uses = [.. connected.Uses] };
        failed.Uses[1] = new AccountUse(AccountCapability.Calendar, CapabilityState.On, UseProblem.Unreachable);
        var unreachable = AccountSettingsRules.Fingerprint(new AccountsSnapshot([failed]));
        Assert.NotEqual(drawn, unreachable);

        failed.Uses[1] = new AccountUse(AccountCapability.Calendar, CapabilityState.On, UseProblem.SignInRefused);
        Assert.NotEqual(unreachable, AccountSettingsRules.Fingerprint(new AccountsSnapshot([failed])));
    }

    [Fact]
    public void AChangeStartingOrEndingRedrawsThePage()
    {
        var free = CorePage.Of(MailboxOnly());
        var drawn = AccountSettingsRules.PageFingerprint(free).ToList();
        Assert.Equal(drawn, AccountSettingsRules.PageFingerprint(CorePage.Of(MailboxOnly())));
        // The page holds and spins from the record alone, so a change running is a redraw.
        var running = free with
        {
            Uses = [free.Uses[0], free.Uses[1] with { On = true, Control = new ControlState(false, true) }, free.Uses[2]],
        };
        Assert.NotEqual(drawn, AccountSettingsRules.PageFingerprint(running));
        Assert.NotEqual(drawn, AccountSettingsRules.PageFingerprint(free with { Remove = new ControlState(false, true) }));
        // The reply setting: drawn, chosen, and running on its own control.
        var asks = free with { ReplyFallback = new AccountReplyFallback(ReplyFallbackChoice.Ask, new ControlState(true, false)) };
        Assert.NotEqual(drawn, AccountSettingsRules.PageFingerprint(asks));
        var never = asks with { ReplyFallback = asks.ReplyFallback! with { Choice = ReplyFallbackChoice.Never } };
        Assert.NotEqual(AccountSettingsRules.PageFingerprint(asks), AccountSettingsRules.PageFingerprint(never));
        Assert.NotEqual(
            AccountSettingsRules.PageFingerprint(never),
            AccountSettingsRules.PageFingerprint(never with { ReplyFallback = never.ReplyFallback! with { Control = new ControlState(false, true) } }));
        // The Sign-in section: a refusal said, and a password being tried.
        var signIn = new AccountSignIn(AccountSignInLine.Password, [], new ControlState(true, false), null, false);
        var section = free with { SignIn = signIn };
        Assert.NotEqual(drawn, AccountSettingsRules.PageFingerprint(section));
        Assert.NotEqual(
            AccountSettingsRules.PageFingerprint(section),
            AccountSettingsRules.PageFingerprint(free with { SignIn = signIn with { Line = AccountSignInLine.Refused } }));
        var field = new AccountPasswordField(AccountSecretKind.PasswordOrToken, true, new ControlState(true, false));
        var typing = free with { SignIn = signIn with { Password = field } };
        Assert.NotEqual(
            AccountSettingsRules.PageFingerprint(typing),
            AccountSettingsRules.PageFingerprint(free with { SignIn = signIn with { Password = field with { Save = new ControlState(false, true) } } }));
        // And a notice raised, which the page says once.
        var notice = new AccountChangeNoticeRaised(1, new AccountChangeNotice.LinkFailed(LinkSlot.Calendar, new AccountChangeRefusal.Other("gone")));
        Assert.NotEqual(drawn, AccountSettingsRules.PageFingerprint(free with { Notice = notice }));
        Assert.Empty(AccountSettingsRules.PageFingerprint(null));
    }
}
