// Settings → Accounts drawn from the core's one snapshot (docs/accounts.md rules 8 and 11 to 13):
// every account listed, mail or not; a link suggested where the calendar server schedules as the
// mailbox's address, made only once the person presses it, and named by its kind beside the mailbox
// of its address, after which the use is a row opening the account it comes from; a use switched off
// only once it is confirmed; a question open on the page kept open when the page is drawn again; a
// removal that names the accounts losing their link; and Stop linking, asked first, on the account
// a use is taken from.
//
// WHY HERE. The rules are pinned in Mailcal.Tests (AccountSettingsRulesTests), but whether the page
// draws them, and whether a press reaches the core and comes back as the next snapshot, is WinUI and
// the core together: a picker wired to nothing draws exactly like one that links.
//
// The launch is stalwart-linked: alice's mailbox used for mail alone, beside alice's and bob's
// calendar and contacts as accounts of their own. Stalwart lists alice's address in her calendar
// principal's calendar-user-address-set, so her calendar is the one suggested for her mailbox. That
// mode connects only its canned accounts, so whatever a case links or switches is gone at the next
// launch. The cases build on one another and run in the order written.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(LinkedHarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class SettingsAccountsTests(LinkedHarnessApp harness)
{
    private void Case(string name, Action<App, SetupFlow> body) =>
        harness.Case(name, app => body(app, new SetupFlow(app)), app => app.CloseDialogs());

    /// <summary>Each account row's help text, which names its kind and what it is linked to.</summary>
    private static List<string> RowHelp(App app, SetupFlow setup) => setup.AccountRows().Select(r => app.Agent.Read(r).Help).ToList();

    private static Element InPage(App app, Element dialog, string id) =>
        app.Agent.Find(new Filter { Id = id, Under = dialog }) ?? throw new InvalidOperationException($"the account page has no #{id}");

    /// <summary>The button labelled with <paramref name="key"/>. It may be below the page's fold, where
    /// a peer reports no size; a collapsed one is in no tree.</summary>
    private static Element Button(App app, Element dialog, string key)
    {
        var labels = Catalog.Everywhere(key);
        Element? found = null;
        return App.Eventually(() => (found = app.Agent.FindAll(new Filter { Type = "Button", Under = dialog })
                .FirstOrDefault(b => labels.Contains(b.Name))) is not null, TimeSpan.FromSeconds(5))
            ? found!
            : throw new InvalidOperationException($"the page offers no '{Catalog.English(key)}'");
    }

    /// <summary>What a use filled from alice's calendar account says it comes from, in every locale.</summary>
    private static IEnumerable<string> FromCalendarAccount() =>
        Catalog.Everywhere("settings_account_use_linked").Select(t => t.Replace("{account}", $"{Harness.Alice} (CalDAV/CardDAV)"));

    [HarnessFact]
    public void EveryAccountIsListedTheTwoWithoutAMailboxIncluded() => Case(nameof(EveryAccountIsListedTheTwoWithoutAMailboxIncluded), (app, setup) =>
    {
        var rows = setup.AccountRows();
        Assert.True(rows.Count == 3,
            $"Settings lists every account from accounts_snapshot (docs/accounts.md rule 11); the mail-only list has one row here, the snapshot three. Listed: {string.Join(" | ", rows.Select(r => r.Name))}");
        var help = RowHelp(app, setup);
        Assert.True(help.Count(h => h.Contains("CalDAV", StringComparison.Ordinal)) == 2,
            $"the two accounts used for calendar and contacts alone are named by their kind. Rows: {string.Join(" | ", help)}");
    });

    [HarnessFact]
    public void TheMailboxsOwnCalendarIsSuggestedAndLinkedOnlyWhenThePersonPressesIt() => Case(nameof(TheMailboxsOwnCalendarIsSuggestedAndLinkedOnlyWhenThePersonPressesIt), (app, setup) =>
    {
        var dialog = setup.OpenAccount(Harness.Alice, 0);
        var picker = InPage(app, dialog, "AccountLink_Calendar");
        Assert.Contains(app.Agent.Read(picker).Selection ?? "", Catalog.Everywhere("settings_account_link_none"));
        // The calendar server is asked once the page first reads the snapshot, so the suggestion
        // arrives a moment after the page.
        var link = app.Agent.Wait(new Filter { Id = "AccountLinkSuggested_Calendar", Under = dialog }, TimeSpan.FromSeconds(15))
            ?? throw new InvalidOperationException("the calendar server schedules as the mailbox's address, so its calendar is suggested with a button that links it");
        // A question left open while the page is drawn again: the link redraws it twice, running and done.
        app.Agent.Invoke(InPage(app, dialog, "AccountRemove"));
        Assert.NotNull(app.Agent.Wait(new Filter { Id = "AccountRemoveConfirm", Under = dialog }, TimeSpan.FromSeconds(5)));
        app.Agent.Invoke(link);
        Assert.True(App.Eventually(() => app.Agent.Find(new Filter { Id = "AccountUseLinked_Calendar", Under = dialog }) is { } now
                && FromCalendarAccount().Any(from => now.Name.EndsWith(from, StringComparison.Ordinal)),
            TimeSpan.FromSeconds(10)),
            "pressing Link fills the calendar from the suggested account, a row naming it by its kind (docs/accounts.md rules 8 and 12): it has the mailbox's address, and the bare address would read as the mailbox itself");
        Assert.True(app.Agent.Find(new Filter { Id = "AccountUse_Calendar", Under = dialog }) is null
            && app.Agent.Find(new Filter { Id = "AccountLink_Calendar", Under = dialog }) is null,
            "a use a link fills is neither a switch, which could only be off, nor a picker (rule 12)");
        Assert.True(app.Agent.Find(new Filter { Id = "AccountLinkSuggested_Calendar", Under = dialog }) is null,
            "a link already made is not suggested again: the page was drawn from the next snapshot");
        Assert.True(App.Eventually(() => app.Agent.Find(new Filter { Id = "AccountRemoveConfirm", Under = dialog }) is { Enabled: true }, TimeSpan.FromSeconds(5)),
            "the removal's question survives the page being drawn again, and can be answered once the link is done: a question that closed by itself would read as answered");
        app.Agent.Invoke(Button(app, dialog, "action_cancel"));
        // From the other end: naming a calendar from the mailbox links the mailbox to the calendar
        // too, and only the next snapshot can say so.
        var help = RowHelp(app, setup);
        Assert.True(help.Count(h => h.Contains($"Mail: {Harness.Alice}", StringComparison.Ordinal)) == 1,
            $"one calendar account names the mailbox it sends through. Rows: {string.Join(" || ", help)}");
        Assert.True(help.Count(h => h.Contains($"Calendar: {Harness.Alice} (CalDAV/CardDAV)", StringComparison.Ordinal)) == 1,
            $"the mailbox's row names its calendar by its kind, not by the address they share. Rows: {string.Join(" || ", help)}");
    });

    [HarnessFact]
    public void SwitchingAUseOffIsAskedFirstAndCancellingPutsTheSwitchBack() => Case(nameof(SwitchingAUseOffIsAskedFirstAndCancellingPutsTheSwitchBack), (app, setup) =>
    {
        var dialog = setup.OpenAccount(Harness.Alice, 1);
        var contacts = InPage(app, dialog, "AccountUse_Contacts");
        Assert.Equal("On", app.Agent.Read(contacts).Toggle);
        app.Agent.Toggle(contacts);
        // Switching a use off deletes what the device holds of it, so it is confirmed first (rule 13).
        Button(app, dialog, "action_switch_off");
        app.Agent.Invoke(Button(app, dialog, "action_cancel"));
        Assert.True(App.Eventually(() => app.Agent.Read(InPage(app, dialog, "AccountUse_Contacts")).Toggle == "On", TimeSpan.FromSeconds(5)),
            "cancelling leaves the use on");
    });

    [HarnessFact]
    public void TheLastUseIsHeldOn() => Case(nameof(TheLastUseIsHeldOn), (app, setup) =>
    {
        var mail = InPage(app, setup.OpenAccount(Harness.Alice, 0), "AccountUse_Mail");
        Assert.Equal("On", app.Agent.Read(mail).Toggle);
        Assert.False(mail.Enabled, "the last of mail, calendar and contacts cannot be switched off, so its switch is not offered (rule 13)");
    });

    [HarnessFact]
    public void RemovingAnAccountNamesTheAccountsThatLoseTheirLinkAndCancelKeepsIt() => Case(nameof(RemovingAnAccountNamesTheAccountsThatLoseTheirLinkAndCancelKeepsIt), (app, setup) =>
    {
        // Linked by the case before: the mailbox names alice's calendar account.
        var dialog = setup.OpenAccount(Harness.Alice, 1);
        app.Agent.Invoke(InPage(app, dialog, "AccountRemove"));
        var message = app.Agent.Wait(new Filter { Id = "AccountRemoveMessage", Under = dialog }, TimeSpan.FromSeconds(5))
            ?? throw new InvalidOperationException("Remove asked nothing before removing the account");
        Assert.True(message.Name.StartsWith($"{Harness.Alice} (CalDAV/CardDAV) ", StringComparison.Ordinal),
            $"the confirmation names the account it removes apart from the mailbox of its address (rule 8). It reads: {message.Name}");
        var unlinks = Catalog.Everywhere("remove_account_unlinks").Select(t => t.Replace("{accounts}", $"{Harness.Alice} (IMAP)"));
        Assert.True(unlinks.Any(sentence => message.Name.Contains(sentence, StringComparison.Ordinal)),
            $"the confirmation names the accounts that lose their link, the mailbox by its kind (rules 8 and 12). It reads: {message.Name}");
        app.Agent.Invoke(Button(app, dialog, "action_cancel"));
        Assert.Equal(3, setup.AccountRows().Count);
    });

    [HarnessFact]
    public void ALinkedCalendarOpensTheAccountItComesFromWhichStopsLinkingOnceAsked() => Case(nameof(ALinkedCalendarOpensTheAccountItComesFromWhichStopsLinkingOnceAsked), (app, setup) =>
    {
        // Linked by the second case: the mailbox's calendar comes from alice's calendar account.
        var dialog = setup.OpenAccount(Harness.Alice, 0);
        app.Agent.Invoke(InPage(app, dialog, "AccountUseLinked_Calendar"));
        var stop = app.Agent.Wait(new Filter { Id = "AccountStopLinking_0", Under = dialog }, TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("the row opened no page listing the mailbox as taking its calendar (rule 12)");
        Assert.True(app.Agent.Find(new Filter { Id = "AccountUseLinked_Calendar", Under = dialog }) is null,
            "the row opened the calendar account's page, whose calendar is its own");
        app.Agent.Invoke(stop);
        var message = app.Agent.Wait(new Filter { Id = "AccountStopLinking_0Message", Under = dialog }, TimeSpan.FromSeconds(5))
            ?? throw new InvalidOperationException("Stop linking asked nothing before stopping");
        Assert.True(Catalog.Everywhere("settings_account_stop_linking_calendar").Any(lost => message.Name.StartsWith(lost, StringComparison.Ordinal)),
            $"the question says the mailbox's calendar will no longer come from this account. It reads: {message.Name}");
        app.Agent.Invoke(InPage(app, dialog, "AccountStopLinking_0Cancel"));
        Assert.NotNull(app.Agent.Wait(new Filter { Id = "AccountStopLinking_0", Under = dialog }, TimeSpan.FromSeconds(5)));
        Assert.Contains(RowHelp(app, setup), help => help.Contains($"Calendar: {Harness.Alice} (CalDAV/CardDAV)", StringComparison.Ordinal));

        dialog = setup.OpenAccount(Harness.Alice, 1);
        app.Agent.Invoke(InPage(app, dialog, "AccountStopLinking_0"));
        app.Agent.Invoke(app.Agent.Wait(new Filter { Id = "AccountStopLinking_0Confirm", Under = dialog }, TimeSpan.FromSeconds(5))
            ?? throw new InvalidOperationException("Stop linking asked nothing before stopping"));
        Assert.True(app.Agent.WaitGone(new Filter { Id = "AccountStopLinking_0", Under = dialog }, TimeSpan.FromSeconds(10)),
            "confirmed, every link between the two stops, and the group goes with its last row");
        Assert.DoesNotContain(RowHelp(app, setup), help => help.Contains($"Calendar: {Harness.Alice} (CalDAV/CardDAV)", StringComparison.Ordinal));
    });
}
