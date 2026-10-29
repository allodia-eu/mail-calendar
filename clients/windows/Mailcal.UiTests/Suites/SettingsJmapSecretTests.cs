// A JMAP account whose pasted password stopped working, given a new one on its page in Settings →
// Accounts (docs/accounts.md rule 11, the expired-sign-in prompt of rule 7): the server refuses the
// stored secret, the window says so, the account's page offers the field, and the new secret
// reconnects the account in place.
//
// WHY HERE. The field is drawn only for an account the core reports as needing to sign in again,
// which only a server refusing a real request produces, and what matters is that the secret typed
// reaches the core and that the core's answer clears the prompt.
//
// The cases set alice's mailbox up through the form, change her password on the server, give the
// account the new one, and put both back; the server's password is restored in each case's cleanup
// whatever happens, because every other harness suite signs in with it. They run in the order
// written, and the last removes the account.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
[Trait("Dataset", "harness")]
public sealed class SettingsJmapSecretTests(SetupHarnessApp harness) : IClassFixture<SetupHarnessApp>
{
    private const string Seeded = "harness-alice-pw";
    private const string Changed = "harness-alice-replaced-secret-pw";

    private void Case(string name, Action<App, SetupFlow> body, bool changesPassword = false) =>
        harness.Case(name, app => body(app, new SetupFlow(app)), app =>
        {
            if (changesPassword)
            {
                Harness.SetPassword("alice", Seeded);
            }
            app.CloseDialogs();
        });

    /// <summary>Asks the server for mail until the window says the account has to sign in again.</summary>
    private static void WaitSignInExpired(App app)
    {
        var expired = App.Eventually(() =>
        {
            app.CloseDialogs();
            if (app.Agent.Find(Filter.ById("SelectionSync", "Button")) is { } sync)
            {
                app.Agent.Invoke(sync);
            }
            return app.Agent.Wait(Filter.ById("SignInExpiredBanner") with { OnScreen = true }, TimeSpan.FromSeconds(5)) is not null;
        }, TimeSpan.FromSeconds(45));
        Assert.True(expired, "the server refuses the stored secret, and the window never said the account has to sign in again");
    }

    /// <summary>Types <paramref name="secret"/> into the account page's field, presses its button and
    /// waits for the answer: the field gone, or a notice saying how it went. Returns the dialog. A
    /// fresh dialog, so the notice waited for cannot be one the last attempt left.</summary>
    private static Element GiveSecret(App app, SetupFlow flow, string secret)
    {
        app.CloseDialogs();
        var dialog = flow.OpenAccount(Harness.Alice);
        var field = app.Agent.Find(new Filter { Id = "AccountReplaceSecret", Under = dialog });
        Assert.True(field is not null,
            "the account's page offers a new secret to a JMAP account whose pasted one stopped working: it has no servers to edit and no provider to sign in at");
        app.Agent.SetValue(field!, secret);
        app.Agent.Invoke(app.Agent.Find(new Filter { Id = "AccountReplaceSecretSave", Under = dialog })!);
        var notices = Catalog.Everywhere("settings_account_signed_in").Concat(Catalog.Everywhere("settings_account_signin_failed")).ToHashSet();
        Assert.True(App.Eventually(
            () => app.Agent.Find(new Filter { Id = "AccountReplaceSecret", Under = dialog }) is null
                || app.Agent.FindAll(new Filter { Type = "Text", Under = dialog }).Any(text => notices.Contains(text.Name)),
            TimeSpan.FromSeconds(30)), "the new secret was given, and the page never said how it went");
        return dialog;
    }

    /// <summary>Asserts the account's page no longer asks for a secret and the window's prompt is gone.</summary>
    private static void AssertSignedIn(App app, SetupFlow flow, string why)
    {
        var dialog = flow.OpenAccount(Harness.Alice);
        Assert.True(app.Agent.Find(new Filter { Id = "AccountReplaceSecret", Under = dialog }) is null,
            $"{why}, so the page no longer asks for one: the core reconnected the account and its next answer cleared the expired sign-in");
        app.CloseDialogs();
        Assert.True(app.Agent.Find(Filter.ById("SignInExpiredBanner") with { OnScreen = true }) is null, $"{why}, so the window's prompt is gone too");
    }

    [HarnessFact]
    public void AJmapMailboxSetUpWithAPasswordConnects() => Case(nameof(AJmapMailboxSetUpWithAPasswordConnects), (app, flow) =>
    {
        flow.OpenManualSetup(Harness.Alice, Filter.ById("JmapChoice"));
        app.Agent.SetValue(app.Agent.Find(Filter.ById("JmapEmail", "Edit"))!, Harness.Alice);
        app.Agent.SetValue(app.Agent.Find(Filter.ById("JmapServer", "Edit"))!, Harness.Base);
        // The server is asked whether it offers a sign-in first; the harness's names an issuer no
        // host resolves, so the answer is the password field.
        var password = app.Agent.Wait(Filter.ById("JmapPassword"), TimeSpan.FromSeconds(20))
            ?? throw new InvalidOperationException("the JMAP form never offered its password field");
        app.Agent.SetValue(password, Seeded);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("ConnectButton", "Button"))!);
        flow.CompleteSetupPrompts();
        AssertSignedIn(app, flow, "the password was right");
    });

    [HarnessFact]
    public void APasswordChangedOnTheServerIsAskedForOnThePageAndTheNewOneReconnects() =>
        Case(nameof(APasswordChangedOnTheServerIsAskedForOnThePageAndTheNewOneReconnects), (app, flow) =>
        {
            Harness.SetPassword("alice", Changed);
            WaitSignInExpired(app);
            GiveSecret(app, flow, Changed);
            AssertSignedIn(app, flow, "the new secret is the one the server holds");
            // And back, through the same field, so the account ends holding the seeded password.
            Harness.SetPassword("alice", Seeded);
            WaitSignInExpired(app);
            GiveSecret(app, flow, Seeded);
            AssertSignedIn(app, flow, "the seeded secret is the one the server holds again");
        }, changesPassword: true);

    [HarnessFact]
    public void AWrongSecretIsRefusedAndThePageKeepsAsking() => Case(nameof(AWrongSecretIsRefusedAndThePageKeepsAsking), (app, flow) =>
    {
        Harness.SetPassword("alice", Changed);
        WaitSignInExpired(app);
        var dialog = GiveSecret(app, flow, "not-the-password");
        var failed = Catalog.Everywhere("settings_account_signin_failed");
        Assert.True(app.Agent.FindAll(new Filter { Type = "Text", Under = dialog }).Any(text => failed.Contains(text.Name)),
            "a secret the server refuses is said, rather than taken as signed in");
        Assert.True(app.Agent.Find(new Filter { Id = "AccountReplaceSecret", Under = dialog }) is not null, "and the field stays, since the account still needs one");
        Harness.SetPassword("alice", Seeded);
        GiveSecret(app, flow, Seeded);
        AssertSignedIn(app, flow, "the seeded secret is the one the server holds again");
    }, changesPassword: true);

    [HarnessFact]
    public void RemovingItReturnsTheWindowToFirstRunSetup() => Case(nameof(RemovingItReturnsTheWindowToFirstRunSetup), (_, flow) =>
        flow.RemoveOnlyAccount(Harness.Alice));
}
