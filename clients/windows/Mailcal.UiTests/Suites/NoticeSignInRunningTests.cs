// A notice's Sign in again while the sign-in it starts is out (docs/notices.md rule 7,
// docs/waiting.md rule 1): the bar's button is held with its spinner from the press until the
// browser has had time to come up, and pressable again while the browser waits, so a tab closed
// by accident is one more press away.
//
// WHY THIS STAGING. Only an account that signs in through a browser is offered Sign in again, and
// the harness's sign-in server (stalwart-oauth, http://localhost:28081) is the one here that runs
// such a sign-in. A JMAP account at it is planted beside the canned harness account, with a client
// registered for it there and a refresh token the server never issued, so the launch's refresh is
// refused by the server itself and the core raises the notice. The press hands the server's login
// page to the OS, which the suites' launch holds back (MAILCAL_NO_EXTERNAL_OPEN); nothing comes back
// from it, and the case ends the sign-in with the account page's Cancel.

using System.Net.Http.Json;
using System.Text.Json.Nodes;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The app on the harness store with an account at the harness's sign-in server beside
/// it, whose stored sign-in the server refuses.</summary>
public sealed class RefusedSignInHarnessApp : HarnessApp
{
    /// <summary>The harness namespace the stalwart launch keeps its accounts in.</summary>
    internal const string Namespace = "dev";

    /// <summary>The planted account's id, in the form the core derives for a JMAP account.</summary>
    internal const string StoredAs = "alice@test.local@jmap:localhost:28081";

    private const string Server = "http://localhost:28081";

    private protected override App Launch()
    {
        StoredAccounts.Plant(Namespace, StoredAs, Config(RegisteredClient()));
        return base.Launch();
    }

    /// <summary>A client the sign-in server knows, registered as the app registers one (RFC 7591),
    /// so the refresh is refused for its token rather than for an unknown client.</summary>
    private static string RegisteredClient()
    {
        using var http = new HttpClient { Timeout = TimeSpan.FromSeconds(10) };
        var answer = http.PostAsJsonAsync($"{Server}/auth/register", new
        {
            client_name = "ui-suite",
            redirect_uris = new[] { Redirect },
            token_endpoint_auth_method = "none",
            // A client registered without them is refused every scope, and the browser comes
            // straight back from the login page with nothing left to wait on.
            scope = string.Join(' ', Scopes),
        }).GetAwaiter().GetResult();
        answer.EnsureSuccessStatusCode();
        var body = JsonNode.Parse(answer.Content.ReadAsStringAsync().GetAwaiter().GetResult());
        return (string?)body?["client_id"] ?? throw new InvalidOperationException("the sign-in server registered no client");
    }

    private static readonly string[] Scopes = ["offline_access", "urn:ietf:params:oauth:scope:mail"];

    // An unpackaged build's scheme (OAuthScheme.Unpackaged).
    private static string Redirect => $"{Brand.AppId}.dev://jmap-oauth";

    private static string Config(string clientId) => $"""
        [jmap]
        base_url = "{Server}"
        email = "{Harness.Alice}"

        [jmap.oauth]
        authorize_endpoint = "{Server}/login"
        client_id = "{clientId}"
        issuer = "{Server}"
        redirect_uri = "{Redirect}"
        refresh_token = "sw1.never-issued-by-this-server"
        resource = "https://localhost:28081/.well-known/jmap"
        scopes = [{string.Join(", ", Scopes.Select(scope => $"\"{scope}\""))}]
        signed_in_as = "{Harness.Alice}"
        token_endpoint = "{Server}/auth/token"
        """;
}

[Trait("Dataset", "harness")]
public sealed class NoticeSignInRunningTests(RefusedSignInHarnessApp harness) : IClassFixture<RefusedSignInHarnessApp>
{
    private static readonly string Expired = App.NoticeWords("notice_signin_expired", Harness.Alice);

    private static Element? Ring(App app) => app.NoticeBar() is { } bar
        ? app.Agent.Find(new Filter { Id = "NoticeAction_Running", Under = bar, OnScreen = true })
        : null;

    private static Element? Action(App app) => app.NoticeBar() is { } bar && app.Says(bar, Expired) ? app.NoticeAction(bar) : null;

    [HarnessFact]
    public void SignInAgainSpinsFromThePressUntilTheBrowserIsUp() => harness.Case(nameof(SignInAgainSpinsFromThePressUntilTheBrowserIsUp), app =>
    {
        app.RowsList();
        var bar = app.WaitNotice(Expired, TimeSpan.FromSeconds(45))
            ?? throw new InvalidOperationException("the sign-in server refused the planted account's refresh token, and the window never said so");
        var action = app.NoticeAction(bar) ?? throw new InvalidOperationException("the expired sign-in's notice offers no action");
        Assert.Equal(Catalog.English("notice_action_sign_in_again"), action.Name);
        Assert.True(Ring(app) is null, "nothing runs before the press");

        app.Agent.Invoke(action);
        Assert.True(App.Eventually(() => Ring(app) is { Rendered: true }, TimeSpan.FromSeconds(20)),
            "from the press, while the browser comes up, the button shows it is working: greying alone is not that");
        Assert.True(App.Eventually(() => Ring(app) is null && Action(app) is { Enabled: true }, TimeSpan.FromSeconds(15)),
            "once the browser has had time to come up, the spinner goes and a press starts the sign-in again");

        // The account page is where a sign-in out in the browser is cancelled.
        var flow = new SetupFlow(app);
        Element? cancel = null;
        for (var index = 0; index < 2 && cancel is null; index++)
        {
            var dialog = flow.OpenAccount(Harness.Alice, index);
            cancel = app.Agent.Wait(new Filter { Id = "AccountSignInCancel", Type = "Button", Under = dialog }, TimeSpan.FromSeconds(3));
            if (cancel is null)
            {
                app.CloseDialogs();
            }
        }
        app.Agent.Invoke(cancel ?? throw new InvalidOperationException("neither of alice's account pages offers to cancel the sign-in out"));
        app.CloseDialogs();
        Assert.True(App.Eventually(() => Ring(app) is null && Action(app) is { Enabled: true }, TimeSpan.FromSeconds(15)),
            "once the sign-in ends Sign in again can still be pressed");
    }, _ => StoredAccounts.Remove(RefusedSignInHarnessApp.Namespace, RefusedSignInHarnessApp.StoredAs));
}
