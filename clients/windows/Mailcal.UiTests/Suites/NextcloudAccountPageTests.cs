// Signing in with Nextcloud against a real Nextcloud server (docs/nextcloud-sign-in.md rules 2, 5,
// 6 and 7): from the manual calendar and contacts form, a calendar and contacts account named by its
// profile address that signs in with Nextcloud; and a typed-password account at the same server,
// whose page offers to sign in with Nextcloud instead, switches, and then signs in again.
//
// WHY HERE. The core's half is the live suite's (live_nextcloud_sign_in.rs). What only the running
// client shows is the whole loop: the browser opened, the poll noticing the grant, the completion
// run off the UI thread, and the page redrawn from what the core then says, none of which a unit
// test or a stand-in that refuses DAV can reach.
//
// The server is the engine's Nextcloud fixture (docker/nextcloud in email-calendar-sync-engine,
// `docker compose up -d --wait`, 127.0.0.1:18082). Each sign-in opens the default browser on the
// fixture's login page, and Driver/NextcloudBrowser.cs does in its place what the person does
// there; close the tabs it leaves. The cases run in the order written, and the last removes the
// account.

using System.Runtime.CompilerServices;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Trait("Dataset", "harness")]
public sealed class NextcloudAccountPageTests(SetupHarnessApp harness) : IClassFixture<SetupHarnessApp>
{
    internal const string Fixture = "http://127.0.0.1:18082";
    private const string User = "alice";
    private const string Password = "nextcloud-alice-pw";

    // The fixture's profile address, which names an account a Nextcloud sign-in makes, and the login
    // a typed-password account is set up with.
    private const string Address = "alice@test.local";

    // Each case starts with the fixture's refused passwords forgotten: another suite's, or an
    // earlier run's, slow every sign-in from this host and then refuse it.
    private void Case(string name, Action<App, SetupFlow> body) =>
        harness.Case(name, app =>
        {
            NextcloudBrowser.ForgetRefusals();
            body(app, new SetupFlow(app));
        }, app => app.CloseDialogs());

    private static Element Field(App app, string id) =>
        app.Agent.Find(Filter.ById(id, "Edit")) ?? throw new InvalidOperationException($"the form has no #{id}");

    /// <summary>The manual calendar and contacts form, at the fixture, with its server field left so
    /// the form asks, and the offer it draws.</summary>
    private static Element OfferedOnTheManualForm(App app, SetupFlow flow, string login, string password)
    {
        flow.OpenManualSetup(Address, Filter.ById("DavChoice"));
        app.Agent.SetValue(Field(app, "DavLogin"), login);
        app.Agent.SetValue(Field(app, "DavPassword"), password);
        app.Agent.SetValue(Field(app, "DavCaldavUrl"), $"{Fixture}/remote.php/dav/");
        app.Agent.Focus(Field(app, "DavCaldavUrl"));
        app.Agent.Focus(Field(app, "DavCarddavUrl"));
        Element? offer = null;
        Assert.True(App.Eventually(() => (offer = app.Agent.Find(Filter.ById("NextcloudSignInButton"))) is { Width: > 0 }, TimeSpan.FromSeconds(15)),
            "a real Nextcloud server typed on the manual form is offered its own sign-in");
        return offer!;
    }

    private static bool Says(App app, Element dialog, string key)
    {
        var text = Catalog.English(key);
        return app.Agent.FindAll(new Filter { Under = dialog }).Any(e => e.Name == text);
    }

    private static bool SaysEventually(App app, Element dialog, string key) =>
        App.Eventually(() => Says(app, dialog, key), TimeSpan.FromSeconds(30));

    [NextcloudFixtureFact]
    public void TheManualFormSignsInWithNextcloudAndTheAccountSaysSo() =>
        Case(nameof(TheManualFormSignsInWithNextcloudAndTheAccountSaysSo), (app, flow) =>
        {
            var offer = OfferedOnTheManualForm(app, flow, string.Empty, string.Empty);
            var before = NextcloudBrowser.NewestLoginToken();
            app.Agent.Invoke(offer);
            NextcloudBrowser.GrantNext(Fixture, before, User, Password);
            // The poll notices the grant, and the account it makes ends setup.
            flow.CompleteSetupPrompts();
            var dialog = flow.OpenAccount(Address);
            Assert.True(SaysEventually(app, dialog, "settings_account_signs_in_nextcloud"),
                "rule 7: an account a Nextcloud sign-in made says it signs in with Nextcloud, never through a provider");
            Assert.True(App.Eventually(() => app.Agent.Find(new Filter { Id = "AccountSignInAgain", Under = dialog }) is not null, TimeSpan.FromSeconds(15)),
                "and offers to sign in again");
            Assert.True(app.Agent.Find(new Filter { Id = "AccountSwitchToPassword", Under = dialog }) is null, "rule 7: it never switches to a typed password");
            Assert.True(app.Agent.Find(new Filter { Id = "AccountServersSave", Under = dialog }) is null,
                "rule 7: Settings edits neither its servers nor its login");
        });

    [NextcloudFixtureFact]
    public void RemovingTheSignedInAccountReturnsTheWindowToFirstRunSetup() =>
        Case(nameof(RemovingTheSignedInAccountReturnsTheWindowToFirstRunSetup), (_, flow) => flow.RemoveOnlyAccount(Address));

    [NextcloudFixtureFact]
    public void ATypedPasswordAccountConnectsBesideTheOffer() => Case(nameof(ATypedPasswordAccountConnectsBesideTheOffer), (app, flow) =>
    {
        OfferedOnTheManualForm(app, flow, Address, Password);
        // The password typed beside it still connects: the offer takes nothing away.
        app.Agent.Invoke(app.Agent.Find(Filter.ById("ConnectButton", "Button"))!);
        flow.CompleteSetupPrompts();
    });

    [NextcloudFixtureFact]
    public void ItsPageOffersToSignInWithNextcloudInsteadOnceItsServerHasSaidSo() =>
        Case(nameof(ItsPageOffersToSignInWithNextcloudInsteadOnceItsServerHasSaidSo), (app, flow) =>
        {
            var dialog = flow.OpenAccount(Address);
            var named = Catalog.English("settings_account_switch_to_provider_named").Replace("{provider}", "Nextcloud", StringComparison.Ordinal);
            Element? toNextcloud = null;
            Assert.True(App.Eventually(() => (toNextcloud = app.Agent.Find(new Filter { Id = "AccountSwitchToProvider", Under = dialog })) is not null, TimeSpan.FromSeconds(15)),
                "rule 2: a calendar and contacts account at Nextcloud offers its sign-in on its page");
            Assert.Equal(named, toNextcloud!.Name);
            Assert.True(app.Agent.Find(new Filter { Id = "AccountSwitchToPassword", Under = dialog }) is null,
                "it signs in with a typed password already");
            Assert.True(Says(app, dialog, "settings_account_signs_in_password"), "its status says it signs in with a password");
        });

    [NextcloudFixtureFact]
    public void SwitchingItToNextcloudThroughTheBrowserRedrawsItsPage() =>
        Case(nameof(SwitchingItToNextcloudThroughTheBrowserRedrawsItsPage), (app, flow) =>
        {
            var dialog = flow.OpenAccount(Address);
            Element? toNextcloud = null;
            App.Eventually(() => (toNextcloud = app.Agent.Find(new Filter { Id = "AccountSwitchToProvider", Under = dialog })) is not null, TimeSpan.FromSeconds(15));
            var before = NextcloudBrowser.NewestLoginToken();
            app.Agent.Invoke(toNextcloud ?? throw new InvalidOperationException("the page offers no switch to Nextcloud"));
            Assert.True(App.Eventually(() => CancelDrawn(app, dialog), TimeSpan.FromSeconds(15)),
                "while the browser holds the sign-in the section draws its Cancel");
            Assert.True(Says(app, dialog, "nextcloud_sign_in_waiting"), "and says the app notices when the person has finished");
            NextcloudBrowser.GrantNext(Fixture, before, User, Password);
            Assert.True(SaysEventually(app, dialog, "settings_account_signs_in_nextcloud"),
                "once the poll has the grant and the core has put it on the account, the page says it signs in with Nextcloud");
        });

    /// <summary>
    /// Whether the Sign-in section draws its Cancel on screen. The section sits below the fold of a
    /// long page: a Cancel scrolled out of view and a collapsed one both read as offscreen, so it
    /// is scrolled to before it is asked about.
    /// </summary>
    private static bool CancelDrawn(App app, Element dialog)
    {
        if (app.Agent.Find(new Filter { Id = "AccountSignInCancel", Under = dialog }) is not { } cancel)
        {
            return false;
        }
        // A button has no scroll item of its own: the page's pane is scrolled instead.
        var pane = app.Agent.Parent(cancel);
        while (pane is not null && pane.Type != "Pane")
        {
            pane = app.Agent.Parent(pane);
        }
        if (pane is not null)
        {
            app.Agent.ScrollToEnd(pane);
        }
        return app.Agent.Find(new Filter { Id = "AccountSignInCancel", Under = dialog }) is { Rendered: true };
    }

    [NextcloudFixtureFact]
    public void ItSignsInWithNextcloudAgainFromItsPage() => Case(nameof(ItSignsInWithNextcloudAgainFromItsPage), (app, flow) =>
    {
        var dialog = flow.OpenAccount(Address);
        Element? again = null;
        Assert.True(App.Eventually(() => (again = app.Agent.Find(new Filter { Id = "AccountSignInAgain", Under = dialog })) is not null, TimeSpan.FromSeconds(15)),
            "rule 8: an account a Nextcloud sign-in made signs in again with Nextcloud");
        var before = NextcloudBrowser.NewestLoginToken();
        app.Agent.Invoke(again!);
        Assert.True(App.Eventually(() => CancelDrawn(app, dialog), TimeSpan.FromSeconds(15)),
            "while the browser holds the sign-in the section draws its Cancel");
        NextcloudBrowser.GrantNext(Fixture, before, User, Password);
        // A sign-in that worked says nothing (docs/waiting.md): the wait ends, and no failure is said.
        Assert.True(App.Eventually(() => app.Agent.Find(new Filter { Id = "AccountSignInCancel", Under = dialog }) is null, TimeSpan.FromSeconds(30)),
            "once the core has put the grant on the account the section no longer waits on the browser");
        Assert.False(Says(app, dialog, "settings_account_signin_failed"), "and the page says nothing failed");
        Assert.True(Says(app, dialog, "settings_account_signs_in_nextcloud"), "it still signs in with Nextcloud");
    });

    [NextcloudFixtureFact]
    public void RemovingItReturnsTheWindowToFirstRunSetup() => Case(nameof(RemovingItReturnsTheWindowToFirstRunSetup), (_, flow) =>
        flow.RemoveOnlyAccount(Address));
}

/// <summary>
/// A case that needs the engine's Nextcloud fixture and a Debug build: skipped, saying why, when
/// either is missing.
/// </summary>
public sealed class NextcloudFixtureFactAttribute : FactAttribute
{
    public NextcloudFixtureFactAttribute([CallerFilePath] string? sourceFilePath = null, [CallerLineNumber] int sourceLineNumber = -1)
        : base(sourceFilePath, sourceLineNumber)
    {
        if (!App.HasBuild())
        {
            Skip = "no Debug build to launch (clients/windows/build-and-run.ps1 -NoRun)";
        }
        else if (!FixtureUp())
        {
            Skip = $"the engine's Nextcloud fixture is not up at {NextcloudAccountPageTests.Fixture} (docker/nextcloud in email-calendar-sync-engine: docker compose up -d --wait)";
        }
    }

    /// <summary>Whether the case needs the Stalwart harness as well, for a mailbox.</summary>
    public bool NeedsHarness
    {
        get => _needsHarness;
        set
        {
            _needsHarness = value;
            if (value && Skip is null && !Harness.IsUp())
            {
                Skip = "the harness is not up (scripts/dev/harness.sh up)";
            }
        }
    }

    private bool _needsHarness;

    private static bool FixtureUp()
    {
        try
        {
            using var http = new HttpClient { Timeout = TimeSpan.FromSeconds(3) };
            return http.GetStringAsync($"{NextcloudAccountPageTests.Fixture}/status.php").GetAwaiter().GetResult()
                .Contains("\"productname\":\"Nextcloud\"", StringComparison.Ordinal);
        }
        catch (Exception e) when (e is HttpRequestException or TaskCanceledException)
        {
            return false;
        }
    }
}
