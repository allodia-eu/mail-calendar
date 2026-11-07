// A mailbox whose calendar and contacts are on a Nextcloud server its own password does not open
// (docs/nextcloud-sign-in.md rule 2, docs/accounts.md rule 18): the separate-server step after the
// connect offers Nextcloud's own sign-in above its password field, and the sign-in moves them to an
// account of their own, linked to the mailbox.
//
// WHY HERE. Which sign-in the form offers is SeparateServerSignIn's, unit-tested in Mailcal.Tests.
// What it cannot see is the step reached at all from a refused calendar, the answer drawn above the
// login, and the grant completed into a linked account.
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

    private static Element Field(App app, string id) =>
        app.Agent.Find(Filter.ById(id, "Edit")) ?? throw new InvalidOperationException($"the form has no #{id}");

    [NextcloudFixtureFact(NeedsHarness = true)]
    public void AMailboxWhoseCalendarServerIsNextcloudIsOfferedItsSignInOnTheSeparateServerStep() =>
        Case(nameof(AMailboxWhoseCalendarServerIsNextcloudIsOfferedItsSignInOnTheSeparateServerStep), (app, flow) =>
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
