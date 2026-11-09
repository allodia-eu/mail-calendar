// A mailbox whose calendar and contacts are on a Nextcloud server its own password does not open
// (docs/nextcloud-sign-in.md rule 2, docs/accounts.md rule 18): the separate-server step after the
// connect offers Nextcloud's own sign-in above its password field, and the sign-in moves them to an
// account of their own, linked to the mailbox. A use skipped there and switched back on later is
// offered the same form from its row on the account's page.
//
// WHY HERE. Which sign-in the form offers is the core's, unit-tested in mailcal-bindings.
// What it cannot see is the step reached at all from a refused calendar, the answer drawn above the
// login, the grant completed into a linked account, and the page's action giving way to its form.
//
// The mailbox is alice's on the Stalwart harness over IMAP; its calendar server is the engine's
// Nextcloud fixture (docker/nextcloud in email-calendar-sync-engine, 127.0.0.1:18082), which refuses
// the harness password, as any separate server would. The sign-in opens the default browser on the
// fixture's login page, and Driver/NextcloudBrowser.cs does in its place what the person does there.
// The cases run in the order written; the last removes both accounts.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Trait("Dataset", "harness")]
public sealed class NextcloudSeparateServerTests(SetupHarnessApp harness) : IClassFixture<SetupHarnessApp>
{
    private const string Address = "alice@test.local";

    private void Case(string name, Action<App, SetupFlow> body) =>
        harness.Case(name, app => body(app, new SetupFlow(app)), app => app.CloseDialogs(), reset: false);

    /// <summary>A case that sets the mailbox up and removes it again whatever happened, so the
    /// cases after it start from the first-run screen.</summary>
    private void Removing(string name, Action<App, SetupFlow> body) =>
        harness.Case(name, app => body(app, new SetupFlow(app)), app =>
        {
            app.CloseDialogs();
            var flow = new SetupFlow(app);
            if (app.Agent.Find(Filter.ById("DetectEmail", "Edit") with { OnScreen = true }) is null
                && flow.AccountRows().Any(row => row.Name == Address))
            {
                flow.RemoveOnlyAccount(Address);
            }
        }, reset: false);

    private static Element Field(App app, string id) =>
        app.Agent.Find(Filter.ById(id, "Edit")) ?? throw new InvalidOperationException($"the form has no #{id}");

    private static Element? InPage(App app, Element dialog, string id) =>
        app.Agent.Find(new Filter { Id = id, Under = dialog });

    /// <summary>Sets alice's mailbox up over IMAP with its calendar at the fixture, as far as the
    /// separate-server step the fixture's refusal leads to.</summary>
    private static void ConnectToTheSeparateServerStep(App app, SetupFlow flow)
    {
        flow.OpenManualSetup(Address, Filter.ById("ImapChoice"));
        app.Agent.SetValue(Field(app, "ImapPort"), "12993");
        app.Agent.SetValue(Field(app, "ImapHost"), "localhost");
        app.Agent.SetValue(Field(app, "Username"), Address);
        app.Agent.SetValue(Field(app, "SmtpPort"), "12465");
        app.Agent.SetValue(Field(app, "SmtpHost"), "localhost");
        app.Agent.SetValue(Field(app, "CaldavUrl"), $"{NextcloudAccountPageTests.Fixture}/remote.php/dav/");
        Element? password = null;
        Assert.True(App.Eventually(() => (password = app.Agent.Find(Filter.ById("Password", "Edit"))) is { Width: > 0 }, TimeSpan.FromSeconds(30)),
            "the harness mailbox takes a password");
        app.Agent.SetValue(password!, "harness-alice-pw");
        Element? connect = null;
        App.Eventually(() => (connect = app.Agent.Find(Filter.ById("ConnectButton", "Button"))) is { Enabled: true }, TimeSpan.FromSeconds(10));
        app.Agent.Invoke(connect ?? throw new InvalidOperationException("the form never let Connect go"));

        Assert.True(app.Agent.Wait(Filter.ById("SeparateServerConnect", "Button"), TimeSpan.FromSeconds(60)) is not null,
            "accounts.md rule 18: a calendar whose server refused the mailbox's password leads to the separate-server step");
    }

    [NextcloudFixtureFact(NeedsHarness = true)]
    public void AUseSwitchedBackOnOffersItsOwnSignInFromItsRowAndItsFormTakesTheActionsPlace() =>
        Removing(nameof(AUseSwitchedBackOnOffersItsOwnSignInFromItsRowAndItsFormTakesTheActionsPlace), (app, flow) =>
        {
            ConnectToTheSeparateServerStep(app, flow);
            app.Agent.Invoke(app.Agent.Find(Filter.ById("SeparateServerSecondary", "Button"))
                ?? throw new InvalidOperationException("the separate-server step offers no Skip"));
            flow.CompleteSetupPrompts();

            var dialog = flow.OpenAccount(Address);
            app.Agent.SetToggle(InPage(app, dialog, "AccountUse_Calendar")
                ?? throw new InvalidOperationException("the account page has no calendar switch"), true);
            Element? action = null;
            Assert.True(App.Eventually(() => (action = InPage(app, dialog, "AccountUseSeparateSignIn_Calendar")) is { Enabled: true }, TimeSpan.FromSeconds(60)),
                "rule 18: a calendar whose server refused the mailbox's password offers a sign-in of its own on its row");
            app.Agent.Invoke(action!);
            Assert.True(app.Agent.Wait(new Filter { Id = "SeparateServerConnect", Under = dialog }, TimeSpan.FromSeconds(10)) is not null,
                "the action opens the form under its row");
            Assert.True(App.Eventually(() => InPage(app, dialog, "AccountUseSeparateSignIn_Calendar") is null, TimeSpan.FromSeconds(5)),
                "the action is hidden while its form is open");

            app.Agent.Invoke(InPage(app, dialog, "SeparateServerSecondary")
                ?? throw new InvalidOperationException("the page's form offers no Cancel"));
            Assert.True(App.Eventually(() => InPage(app, dialog, "AccountUseSeparateSignIn_Calendar") is { Enabled: true }
                && InPage(app, dialog, "SeparateServerConnect") is null, TimeSpan.FromSeconds(10)),
                "Cancel closes the form and puts the action back");
        });

    [NextcloudFixtureFact(NeedsHarness = true)]
    public void AMailboxWhoseCalendarServerIsNextcloudIsOfferedItsSignInOnTheSeparateServerStep() =>
        Case(nameof(AMailboxWhoseCalendarServerIsNextcloudIsOfferedItsSignInOnTheSeparateServerStep), (app, flow) =>
        {
            ConnectToTheSeparateServerStep(app, flow);
            Element? signIn = null;
            Assert.True(App.Eventually(() => (signIn = app.Agent.Find(Filter.ById("SeparateServerSignIn", "Button"))) is { Width: > 0 }, TimeSpan.FromSeconds(15)),
                "rule 2: a separate server that is Nextcloud offers its own sign-in above the password field");
            Assert.Equal(Catalog.English("nextcloud_sign_in_button"), signIn!.Name);
            Assert.True(signIn.Y < Field(app, "SeparateServerUsername").Y, "above the login and password");

            var before = NextcloudBrowser.NewestLoginToken();
            app.Agent.Invoke(signIn);
            Assert.True(App.Eventually(() => app.Agent.Find(Filter.ById("SeparateServerSignInCancel", "Button")) is { Width: > 0 }, TimeSpan.FromSeconds(10)),
                "the wait draws its Cancel");
            NextcloudBrowser.GrantNext(NextcloudAccountPageTests.Fixture, before, "alice", "nextcloud-alice-pw");
            Assert.True(app.Agent.WaitGone(Filter.ById("SeparateServerConnect", "Button"), TimeSpan.FromSeconds(60)),
                "the grant moves the calendar and contacts to an account of their own, and setup goes on");
            flow.CompleteSetupPrompts();
            Assert.True(App.Eventually(() => flow.AccountRows().Count(row => row.Name == Address) == 2, TimeSpan.FromSeconds(15)),
                "the mailbox and the calendar and contacts account beside it");
        });

    [NextcloudFixtureFact(NeedsHarness = true)]
    public void RemovingBothReturnsTheWindowToFirstRunSetup() => Case(nameof(RemovingBothReturnsTheWindowToFirstRunSetup), (app, flow) =>
    {
        while (flow.AccountRows().Count(row => row.Name == Address) > 1)
        {
            var dialog = flow.OpenAccount(Address);
            app.Agent.Invoke(app.Agent.Find(new Filter { Id = "AccountRemove", Under = dialog })!);
            app.Agent.Invoke(app.Agent.Wait(new Filter { Id = "AccountRemoveConfirm", Under = dialog }, TimeSpan.FromSeconds(10))!);
            app.Agent.Settle();
        }
        app.CloseDialogs();
        flow.RemoveOnlyAccount(Address);
    });
}
