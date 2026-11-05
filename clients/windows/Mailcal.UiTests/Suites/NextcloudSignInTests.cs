// Signing in with Nextcloud from the manual calendar and contacts form (docs/nextcloud-sign-in.md
// rules 2, 5 and 6): the offer drawn above the fields once the server typed there answers as
// Nextcloud, the wait and its Cancel, and a grant that became no account kept for the next press and
// withdrawn when setup is left.
//
// WHY HERE. Which server a press signs in at, and the flight's poll, are unit-tested in
// Mailcal.Tests (SetupNextcloudTests, NextcloudFlightTests). What neither sees is the form: a panel
// left collapsed, a LostFocus never wired, a field cleared by the redraw, or a Cancel the footer
// never draws, all of which render without an error.
//
// The server is a stand-in in this process (Driver/NextcloudStandIn.cs), which records what the app
// asked it. The cases that press the sign-in open the person's default browser on the stand-in's
// own page, so they skip on a CI runner, which has no browser to open; close the tabs they leave.
// The found card needs a domain whose detection finds a Nextcloud server, which no loopback server
// stands in for: verify it by hand against a real one.

using System.Runtime.CompilerServices;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Trait("Dataset", "first-run")]
public sealed class NextcloudSignInTests(FirstRunApp firstRun) : IClassFixture<FirstRunApp>
{
    // The core's own interval (mailcal-nextcloud POLL_INTERVAL), with room for a slow ask.
    private static readonly TimeSpan PollInterval = TimeSpan.FromSeconds(2);

    private void Case(string name, Action<App> body) =>
        firstRun.Case(name, body, app =>
        {
            // Back to the address step, which is where setup is left from.
            if (app.Agent.Find(Filter.ById("BackButton", "Button") with { OnScreen = true }) is { Enabled: true } back)
            {
                app.Agent.Invoke(back);
                app.Agent.Settle();
            }
        });

    private static Element Field(App app, string id) =>
        app.Agent.Find(Filter.ById(id, "Edit")) ?? throw new InvalidOperationException($"the form has no #{id}");

    private static string Value(App app, string id) => app.Agent.Read(Field(app, id)).Value ?? "";

    /// <summary>The manual calendar and contacts form, filled in with <paramref name="server"/> as
    /// its calendar server, and the field left, which is when the form asks.</summary>
    private static void FillManualForm(App app, string server)
    {
        new SetupFlow(app).OpenManualSetup("alice@example.org", Filter.ById("DavChoice"));
        app.Agent.SetValue(Field(app, "DavLogin"), "alice");
        app.Agent.SetValue(Field(app, "DavPassword"), "typed-password");
        app.Agent.SetValue(Field(app, "DavCaldavUrl"), server);
        Leave(app);
    }

    private static void Leave(App app)
    {
        app.Agent.Focus(Field(app, "DavCaldavUrl"));
        app.Agent.Focus(Field(app, "DavCarddavUrl"));
    }

    /// <summary>The element <paramref name="id"/> as drawn: a collapsed one keeps its peer with an
    /// empty rectangle, so presence alone is not the test.</summary>
    private static Element? Shown(App app, string id) =>
        app.Agent.Find(Filter.ById(id)) is { Width: > 0, Height: > 0 } shown ? shown : null;

    private static Element Offer(App app)
    {
        Element? offer = null;
        return App.Eventually(() => (offer = Shown(app, "NextcloudSignInButton")) is not null, TimeSpan.FromSeconds(15))
            ? offer!
            : throw new InvalidOperationException("the manual form never offered \"Sign in with Nextcloud\" for a server that answers as Nextcloud");
    }

    [OfflineFact]
    public void AServerThatAnswersAsNextcloudIsOfferedAboveTheFieldsWhichKeepWhatWasTyped() =>
        Case(nameof(AServerThatAnswersAsNextcloudIsOfferedAboveTheFieldsWhichKeepWhatWasTyped), app =>
        {
            using var server = new NextcloudStandIn();
            FillManualForm(app, server.Dav);
            var button = Offer(app);
            Assert.True(Shown(app, "NextcloudNote") is not null, "the line that explains the sign-in goes with it");
            Assert.True(button.Y < Field(app, "DavLogin").Y, "rule 2: the offer is drawn above the fields");
            Assert.Equal("alice", Value(app, "DavLogin"));
            Assert.Equal("typed-password", Value(app, "DavPassword"));
            Assert.Equal(server.Dav, Value(app, "DavCaldavUrl"));
            Assert.True(Shown(app, "ConnectButton") is not null,
                "on the manual form the password route stays as it was, Connect included");
            Assert.Equal(1, server.Count(NextcloudStandIn.Status));

            // Leaving a field again asks nothing new: the form asks once per server.
            Leave(app);
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromSeconds(1));
            Assert.Equal(1, server.Count(NextcloudStandIn.Status));
            Assert.True(Shown(app, "DavUsePasswordButton") is null,
                "the manual form hides no password, so there is nothing to choose it with");
        });

    [OfflineFact]
    public void AServerThatIsNotNextcloudIsOfferedNothing() =>
        Case(nameof(AServerThatIsNotNextcloudIsOfferedNothing), app =>
        {
            using var server = new NextcloudStandIn(nextcloud: false);
            FillManualForm(app, server.Dav);
            Assert.True(App.Eventually(() => server.Count(NextcloudStandIn.Status) == 1, TimeSpan.FromSeconds(10)),
                "leaving the calendar field asks its server whether it is Nextcloud");
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromSeconds(1));
            Assert.True(Shown(app, "NextcloudSignInButton") is null, "rule 1: a server that does not say it is Nextcloud is offered the password alone");
        });

    [BrowserFact]
    public void TheWaitIsSaidWithACancelAtOnceAndCancelStopsThePoll() =>
        Case(nameof(TheWaitIsSaidWithACancelAtOnceAndCancelStopsThePoll), app =>
        {
            using var server = new NextcloudStandIn();
            FillManualForm(app, server.Dav);
            app.Agent.Invoke(Offer(app));
            Assert.True(App.Eventually(() => Shown(app, "NextcloudWaitingText") is not null, TimeSpan.FromSeconds(10)),
                "rule 5: while the browser holds the sign-in the form says to continue there");
            var cancel = Shown(app, "CancelButton");
            Assert.True(cancel is { Enabled: true }, "onboarding.md: the wait has a way out, drawn with it");
            Assert.True(App.Eventually(() => server.Count(NextcloudStandIn.Poll) >= 2, PollInterval * 4),
                $"the app asks the server again every interval; it asked {server.Count(NextcloudStandIn.Poll)} time(s)");

            app.Agent.Invoke(cancel!);
            Assert.True(App.Eventually(() => Shown(app, "NextcloudWaitingText") is null, TimeSpan.FromSeconds(5)), "Cancel takes the wait away");
            Assert.True(App.Eventually(() => Shown(app, "NextcloudSignInButton") is { Enabled: true }, TimeSpan.FromSeconds(5)),
                "and leaves the form, its sign-in ready to press again");
            var polls = server.Count(NextcloudStandIn.Poll);
            Thread.Sleep(PollInterval * 2);
            Assert.Equal(polls, server.Count(NextcloudStandIn.Poll));
        });

    [BrowserFact]
    public void AGrantThatMadeNoAccountIsTriedAgainWithoutTheBrowserAndWithdrawnWhenSetupIsLeft() =>
        Case(nameof(AGrantThatMadeNoAccountIsTriedAgainWithoutTheBrowserAndWithdrawnWhenSetupIsLeft), app =>
        {
            // The stand-in grants at once and refuses the app password at its DAV endpoint, so the
            // grant becomes no account.
            using var server = new NextcloudStandIn { Granting = true };
            FillManualForm(app, server.Dav);
            app.Agent.Invoke(Offer(app));
            Assert.True(App.Eventually(() => server.Count("PROPFIND ") > 0, TimeSpan.FromSeconds(20)),
                "the grant is completed: the app connects with the password it carries");
            var button = Offer(app);
            Assert.True(App.Eventually(() => Shown(app, "NextcloudSignInButton") is { Enabled: true }, TimeSpan.FromSeconds(20)),
                "a completion that failed hands the form back");
            Assert.Equal(1, server.Count(NextcloudStandIn.Begin));
            Assert.Equal(0, server.Count(NextcloudStandIn.Withdraw));

            var connects = server.Count("PROPFIND ");
            app.Agent.Invoke(button);
            Assert.True(App.Eventually(() => server.Count("PROPFIND ") > connects, TimeSpan.FromSeconds(20)),
                "rule 6: the next press tries the kept grant again");
            App.Eventually(() => Shown(app, "NextcloudSignInButton") is { Enabled: true }, TimeSpan.FromSeconds(20));
            Assert.True(server.Count(NextcloudStandIn.Begin) == 1, "without a second trip to the browser: no second sign-in was started");

            app.Agent.Invoke(app.Agent.Find(Filter.ById("BackButton", "Button"))!);
            Assert.True(App.Eventually(() => server.Count(NextcloudStandIn.Withdraw) == 1, TimeSpan.FromSeconds(10)),
                "rule 6: leaving the form withdraws the grant that became no account");
        });
}

/// <summary>
/// A case that opens the person's default browser: it needs a Debug build and a desktop with a
/// browser, so it skips on a CI runner, saying why.
/// </summary>
public sealed class BrowserFactAttribute : FactAttribute
{
    public BrowserFactAttribute([CallerFilePath] string? sourceFilePath = null, [CallerLineNumber] int sourceLineNumber = -1)
        : base(sourceFilePath, sourceLineNumber)
    {
        if (!App.HasBuild())
        {
            Skip = "no Debug build to launch (clients/windows/build-and-run.ps1 -NoRun)";
        }
        else if (Environment.GetEnvironmentVariable("CI") == "true")
        {
            Skip = "opens the default browser, which a CI runner has none of";
        }
    }
}
