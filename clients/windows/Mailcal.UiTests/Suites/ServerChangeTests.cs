// A calendar and address book whose servers were looked for again and found on another site
// (docs/account-autodetect.md rule 14): the uses pause, the account's page says so under each
// switch, and "Review" asks inline, because Settings is itself a dialog. Allowing a server that
// refuses leaves the pause; allowing one that connects moves the account onto it.
//
// WHY HERE. Which note a problem earns and the question's copy are pinned in Mailcal.Tests. What
// neither can see is the inline question: drawn inside a dialog that redraws from every snapshot,
// it has to survive the redraw an allow causes and land its answer on the panel drawn last.
//
// WHY THIS STAGING. A question names two sites, and a loopback server has no site the Public Suffix
// List can place, so a real lookup against the harness never asks. MAILCAL_FAKE_SERVER_CHANGE
// substitutes the lookup's answer and nothing else (ServerChangeHarnessApp). The lookup still has to
// run, which takes a dial the server refuses, so alice's password is changed on the server and the
// app relaunched: a refused sync raises the sign-in prompt and dials nothing, since a password
// account is never sent the same secret again, and a server that does not answer is an outage,
// which is never looked up again. With the account's own sign-in refused, the calendar screen
// leaves its line out for the account's own prompt (use_outages.rs), so the account's page is the
// surface asked about here.
//
// The cases run in the order written, put alice's password back in their cleanup whatever happens,
// and the last removes the account.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
[Trait("Dataset", "harness")]
public sealed class ServerChangeTests(ServerChangeHarnessApp harness) : IClassFixture<ServerChangeHarnessApp>
{
    private const string Seeded = "harness-alice-pw";
    private const string Changed = "harness-alice-server-change-pw";

    private void Case(string name, Action<App, SetupFlow> body, bool changesPassword = false) =>
        harness.Case(name, app => body(app, new SetupFlow(app)), app =>
        {
            if (changesPassword)
            {
                Harness.SetPassword("alice", Seeded);
            }
            app.CloseDialogs();
        });

    private static Element? Review(App app, Element dialog, string use) =>
        app.Agent.Find(new Filter { Id = $"AccountUseServerChangeReview_{use}", Under = dialog });

    private static bool Says(App app, Element under, string key)
    {
        var values = Catalog.Everywhere(key);
        return app.Agent.FindAll(new Filter { Under = under }).Any(e => values.Contains(e.Name));
    }

    /// <summary>Opens the calendar, which asks for it again, until the account's page offers a
    /// review of the calendar's server, and returns that page.</summary>
    private static Element WaitPaused(App app, SetupFlow flow)
    {
        Element? dialog = null;
        var paused = App.Eventually(() =>
        {
            app.CloseDialogs();
            app.Agent.Invoke(app.Agent.Find(Filter.ById("NavCalendar", "ListItem"))!);
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), cap: TimeSpan.FromSeconds(5));
            dialog = flow.OpenAccount(Harness.Alice);
            return Review(app, dialog, "Calendar") is not null;
        }, TimeSpan.FromSeconds(60));
        Assert.True(paused, "the calendar server refused the sign-in, the lookup found it on another site, and the account's page never offered to review it");
        return dialog!;
    }

    /// <summary>Presses Review under the calendar and waits for the question it draws. The question
    /// is a layout panel, which has no automation peer, so it is found by its parts.</summary>
    private static void OpenQuestion(App app, Element dialog)
    {
        app.Agent.Invoke(Review(app, dialog, "Calendar")!);
        if (app.Agent.Wait(new Filter { Id = "ServerChangeAllow", Under = dialog }, TimeSpan.FromSeconds(10)) is null)
        {
            throw new InvalidOperationException("Review drew no question on the account's page");
        }
    }

    private static Element Button(App app, Element dialog, string id) =>
        app.Agent.Find(new Filter { Id = id, Type = "Button", Under = dialog })
            ?? throw new InvalidOperationException($"the question has no #{id}");

    [HarnessFact]
    public void ACalendarAndContactsAccountOnTheHarnessConnects() => Case(nameof(ACalendarAndContactsAccountOnTheHarnessConnects), (app, flow) =>
    {
        flow.OpenManualSetup(Harness.Alice, Filter.ById("DavChoice"));
        app.Agent.SetValue(app.Agent.Find(Filter.ById("DavLogin", "Edit"))!, Harness.Alice);
        app.Agent.SetValue(app.Agent.Find(Filter.ById("DavPassword", "Edit"))!, Seeded);
        app.Agent.SetValue(app.Agent.Find(Filter.ById("DavCaldavUrl", "Edit"))!, Harness.Base);
        app.Agent.SetValue(app.Agent.Find(Filter.ById("DavCarddavUrl", "Edit"))!, Harness.Base);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("ConnectButton", "Button"))!);
        flow.CompleteSetupPrompts();
        var dialog = flow.OpenAccount(Harness.Alice);
        Assert.True(Review(app, dialog, "Calendar") is null && Review(app, dialog, "Contacts") is null,
            "a calendar that connects is not paused, whatever the lookup would say: it is asked only once the server stops answering");
    });

    [HarnessFact]
    public void ARefusedSignInLooksAgainAndAServerOnAnotherSiteIsAskedAbout() =>
        Case(nameof(ARefusedSignInLooksAgainAndAServerOnAnotherSiteIsAskedAbout), (app, flow) =>
        {
            Harness.SetPassword("alice", Changed);
            harness.RelaunchOnFirstRunStore();
            app = harness.App;
            flow = new SetupFlow(app);
            var dialog = WaitPaused(app, flow);
            Assert.True(Says(app, dialog, "settings_account_use_server_changed_calendar"), "the calendar's switch says it is paused for a new server");
            Assert.True(Review(app, dialog, "Contacts") is not null && Says(app, dialog, "settings_account_use_server_changed_contacts"),
                "and so does the address book's, which the same lookup moved");
            OpenQuestion(app, dialog);
            var before = app.Agent.Find(new Filter { Id = "ServerChangeBefore", Under = dialog })?.Name ?? "";
            var now = app.Agent.Find(new Filter { Id = "ServerChangeNow", Under = dialog })?.Name ?? "";
            Assert.True(before.Contains("127.0.0.1", StringComparison.Ordinal) && now.Contains("localhost", StringComparison.Ordinal),
                $"the question names the site the account used and the one it was found on (before: '{before}', now: '{now}')");

            // The password is still the changed one, so the new server refuses it too.
            app.Agent.Invoke(Button(app, dialog, "ServerChangeAllow"));
            var failed = Catalog.Everywhere("server_changed_failed");
            Assert.True(App.Eventually(
                () => app.Agent.Find(new Filter { Id = "ServerChangeError", Under = dialog }) is { } error
                    && app.Agent.FindAll(new Filter { Under = error }).Any(e => failed.Contains(e.Name)), TimeSpan.FromSeconds(30)),
                "the new server refuses the stored password too, and the question says nothing changed where it was asked");
            Assert.True(Says(app, dialog, "settings_account_use_server_changed_calendar"), "and the calendar stays paused");
            app.Agent.Invoke(Button(app, dialog, "ServerChangeNotNow"));
            Assert.True(app.Agent.WaitGone(new Filter { Id = "ServerChangeAllow", Under = dialog }, TimeSpan.FromSeconds(10)), "Not now puts the question away");
            Assert.True(Review(app, dialog, "Calendar") is not null, "and Review is offered again, since the calendar still waits on the person");
        }, changesPassword: true);

    [HarnessFact]
    public void AllowingAServerThatConnectsMovesTheAccountOntoIt() => Case(nameof(AllowingAServerThatConnectsMovesTheAccountOntoIt), (app, flow) =>
    {
        // The case before put the seeded password back, and nothing has dialled the calendar since,
        // so it is still paused.
        var dialog = flow.OpenAccount(Harness.Alice);
        OpenQuestion(app, dialog);
        app.Agent.Invoke(Button(app, dialog, "ServerChangeAllow"));
        Assert.True(App.Eventually(() =>
            {
                dialog = flow.OpenAccount(Harness.Alice);
                return Review(app, dialog, "Calendar") is null && Review(app, dialog, "Contacts") is null;
            }, TimeSpan.FromSeconds(30)),
            "allowing a server that connects lifts the pause on both uses it moved");
        var servers = app.Agent.FindAll(new Filter { Type = "Edit", Under = dialog }).Select(e => app.Agent.Read(e).Value ?? "").ToList();
        Assert.True(servers.Count(value => value == ServerChangeHarnessApp.Moved) == 2,
            $"the account's calendar and address-book servers are the allowed one now (fields: {string.Join(" | ", servers)})");
    });

    [HarnessFact]
    public void RemovingItReturnsTheWindowToFirstRunSetup() => Case(nameof(RemovingItReturnsTheWindowToFirstRunSetup), (_, flow) =>
        flow.RemoveOnlyAccount(Harness.Alice));
}
