// What Settings → Accounts says about an account, from a snapshot entry (docs/accounts.md rules
// 11 to 13), and what an edit of its servers sends back (rule 14).

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
        Uses: uses.Select(use => new AccountUse(use.Item1, use.Item2)).ToArray(),
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
    public void TheLastUseCannotBeSwitchedOff()
    {
        var entry = MailboxOnly();
        Assert.Equal(
            new UseSwitch(Active: true, Asks: false, Enabled: false, Note: UseNote.Last),
            AccountSettingsRules.Switch(entry, AccountCapability.Mail));
        Assert.Equal(
            new UseSwitch(Active: false, Asks: false, Enabled: true, Note: UseNote.None),
            AccountSettingsRules.Switch(entry, AccountCapability.Calendar));
    }

    [Fact]
    public void ColleaguesWaitForContacts()
    {
        var entry = Entry(
            AccountKind.Microsoft,
            (AccountCapability.Mail, CapabilityState.On),
            (AccountCapability.Calendar, CapabilityState.On),
            (AccountCapability.Contacts, CapabilityState.Off),
            (AccountCapability.Colleagues, CapabilityState.Off));
        var colleagues = AccountSettingsRules.Switch(entry, AccountCapability.Colleagues);
        Assert.False(colleagues.Enabled);
        Assert.Equal(UseNote.NeedsContacts, colleagues.Note);

        entry.Uses[2] = new AccountUse(AccountCapability.Contacts, CapabilityState.On);
        Assert.True(AccountSettingsRules.Switch(entry, AccountCapability.Colleagues).Enabled);
    }

    [Fact]
    public void AUseWaitingOnPermissionIsDrawnOffAndSwitchingItOnAsks()
    {
        var entry = Entry(
            AccountKind.Google,
            (AccountCapability.Mail, CapabilityState.On),
            (AccountCapability.Calendar, CapabilityState.NeedsPermission),
            (AccountCapability.Contacts, CapabilityState.Off),
            (AccountCapability.Colleagues, CapabilityState.Off));
        var calendar = AccountSettingsRules.Switch(entry, AccountCapability.Calendar);
        Assert.False(calendar.Active);
        Assert.True(calendar.Asks && calendar.Enabled);
        Assert.Equal(UseNote.Withheld, calendar.Note);
        Assert.True(AccountSettingsRules.NeedsPermission(entry));
        Assert.Equal([AccountCapability.Calendar], AccountSettingsRules.Withheld(entry));
        // Listed as a use: it is chosen, only not granted.
        Assert.Contains(AccountCapability.Calendar, AccountSettingsRules.InUse(entry));
    }

    [Fact]
    public void AnOnlyUseWaitingOnPermissionCanStillBeAskedFor()
    {
        var entry = Entry(
            AccountKind.Google,
            (AccountCapability.Mail, CapabilityState.Off),
            (AccountCapability.Calendar, CapabilityState.NeedsPermission),
            (AccountCapability.Contacts, CapabilityState.Off));
        Assert.Equal(
            new UseSwitch(Active: false, Asks: true, Enabled: true, Note: UseNote.Withheld),
            AccountSettingsRules.Switch(entry, AccountCapability.Calendar));
    }

    [Fact]
    public void SigningInAgainIsOfferedWhereTheProviderSignsIn()
    {
        Assert.True(AccountSettingsRules.SignsInAtProvider(AccountKind.Microsoft));
        Assert.True(AccountSettingsRules.SignsInAtProvider(AccountKind.Google));
        foreach (var kind in new[] { AccountKind.Imap, AccountKind.Dav, AccountKind.Jmap })
        {
            Assert.False(AccountSettingsRules.SignsInAtProvider(kind));
        }
    }

    [Fact]
    public void TheRowNamesTheUsesInOrderAndTheLinks()
    {
        var entry = MailboxOnly() with
        {
            Links = new AccountLinksView(Linked("alice@cloud.example"), null, null),
        };
        Assert.Equal([AccountCapability.Mail], AccountSettingsRules.InUse(entry));
        Assert.Equal([(AccountCapability.Calendar, "alice@cloud.example")], AccountSettingsRules.Linked(entry));
    }

    [Fact]
    public void APickerIsOfferedOnlyForASlotTheAccountCanHold()
    {
        var entry = MailboxOnly();
        Assert.Empty(AccountSettingsRules.LinkPickers(entry));

        entry = entry with
        {
            LinkCandidates = new LinkCandidates([Linked("a@cloud.example"), Linked("b@cloud.example")], [], [], []),
            Links = new AccountLinksView(Linked("b@cloud.example"), null, null),
        };
        var picker = Assert.Single(AccountSettingsRules.LinkPickers(entry));
        Assert.Equal(LinkSlot.Calendar, picker.Slot);
        Assert.Equal(2, picker.Options.Count);
        Assert.Equal(1, picker.Selected);
    }

    [Fact]
    public void ALinkedAccountMissingFromTheCandidatesIsStillShown()
    {
        var entry = MailboxOnly() with
        {
            Links = new AccountLinksView(null, Linked("book@cloud.example"), null),
        };
        var picker = Assert.Single(AccountSettingsRules.LinkPickers(entry));
        Assert.Equal(LinkSlot.Contacts, picker.Slot);
        Assert.Equal([Linked("book@cloud.example")], picker.Options);
        Assert.Equal(0, picker.Selected);
    }

    [Fact]
    public void ASuggestionIsOfferedOnlyWhereNothingIsLinked()
    {
        var entry = MailboxOnly() with
        {
            LinkCandidates = new LinkCandidates(
                [Linked("a@cloud.example"), Linked("b@cloud.example")], [], [], ["b@cloud.example"]),
        };
        Assert.Equal(1, AccountSettingsRules.LinkPickers(entry)[0].Suggested);

        entry = entry with { Links = new AccountLinksView(Linked("a@cloud.example"), null, null) };
        // A link already made is not second-guessed.
        Assert.Null(AccountSettingsRules.LinkPickers(entry)[0].Suggested);
    }

    [Fact]
    public void RemovingNamesTheAccountsThatLoseTheirLink()
    {
        var entry = MailboxOnly();
        Assert.Null(AccountSettingsRules.Unlinked(entry));
        entry = entry with { LinkedFrom = [Linked("a@example.org"), Linked("b@example.org")] };
        Assert.Equal("a@example.org, b@example.org", AccountSettingsRules.Unlinked(entry));
    }

    [Fact]
    public void SwitchingOnAUseTheProviderWithholdsAsksForIt()
    {
        Assert.Equal(UseChangeOutcome.AskProvider, AccountSettingsRules.Outcome(CapabilityChange.NeedsConsent));
        Assert.Equal(UseChangeOutcome.NeedsEndpoint, AccountSettingsRules.Outcome(CapabilityChange.NeedsEndpoint));
        Assert.Equal(UseChangeOutcome.Applied, AccountSettingsRules.Outcome(CapabilityChange.Applied));
    }

    [Fact]
    public void ASignalThatChangedNothingOnThePageLeavesItAlone()
    {
        var snapshot = new AccountsSnapshot([MailboxOnly()]);
        var drawn = AccountSettingsRules.Fingerprint(snapshot, []);
        Assert.Equal(drawn, AccountSettingsRules.Fingerprint(new AccountsSnapshot([MailboxOnly()]), []));

        // A suggestion arriving, a link, and an expired sign-in each change what the page shows.
        var suggested = MailboxOnly() with
        {
            LinkCandidates = new LinkCandidates([Linked("a@cloud.example")], [], [], ["a@cloud.example"]),
        };
        Assert.NotEqual(drawn, AccountSettingsRules.Fingerprint(new AccountsSnapshot([suggested]), []));
        Assert.NotEqual(drawn, AccountSettingsRules.Fingerprint(snapshot, [MailboxOnly().Id]));
        // So does a mailbox listed in the mail settings after the page was drawn, which is where
        // its Mail section comes from.
        Assert.NotEqual(drawn, AccountSettingsRules.Fingerprint(snapshot, [], ["mail:" + MailboxOnly().Id]));
    }
}
