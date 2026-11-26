// The manual form's Nextcloud type (docs/nextcloud-sign-in.md rule 2): a pane that asks for the
// server's address alone, its sign-in held until it holds one, the wait while the core asks the
// address and then while the browser holds the sign-in, and, where the core could not tell the
// address is Nextcloud, the offer to try anyway while the field still holds that address.
//
// WHY HERE. What a press starts and what each answer offers are unit-tested in Mailcal.Tests
// (SetupNextcloudAddressTests). What neither sees is the pane: a type the picker never draws, a
// field the pane leaves out, an offer left collapsed, or a field cleared when a sign-in ends.
//
// The server is a stand-in in this process (Driver/NextcloudStandIn.cs). Its DAV endpoint is a PHP
// path, so the core asks its status and finds Nextcloud; its bare root shows nothing either way.
// The suites' launch holds back the browser (MAILCAL_NO_EXTERNAL_OPEN), so no case opens one.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Trait("Dataset", "first-run")]
public sealed class NextcloudManualSetupTests(FirstRunApp firstRun) : IClassFixture<FirstRunApp>
{
    private void Case(string name, Action<App> body) =>
        firstRun.Case(name, body, app =>
        {
            // Back to the address step, which is where setup is left from.
            if (app.Agent.Find(Filter.ById("CancelButton", "Button") with { OnScreen = true }) is { Enabled: true } cancel)
            {
                app.Agent.Invoke(cancel);
                app.Agent.Settle();
            }
            if (app.Agent.Find(Filter.ById("BackButton", "Button") with { OnScreen = true }) is { Enabled: true } back)
            {
                app.Agent.Invoke(back);
                app.Agent.Settle();
            }
        });

    private static Element Address(App app) =>
        app.Agent.Find(Filter.ById("NextcloudServer", "Edit")) ?? throw new InvalidOperationException("the Nextcloud pane has no #NextcloudServer");

    /// <summary>The element <paramref name="id"/> as drawn: a collapsed one keeps its peer with an
    /// empty rectangle, so presence alone is not the test.</summary>
    private static Element? Shown(App app, string id) =>
        app.Agent.Find(Filter.ById(id)) is { Width: > 0, Height: > 0 } shown ? shown : null;

    private static void OpenPane(App app) =>
        new SetupFlow(app).OpenManualSetup("alice@example.org", Filter.ById("NextcloudChoice"));

    private static Element SignIn(App app) =>
        Shown(app, "NextcloudAddressSignInButton") ?? throw new InvalidOperationException("the Nextcloud pane draws no sign-in");

    private static bool Before(Element first, Element second) =>
        first.Y < second.Y - (first.Height / 2) || (Math.Abs(first.Y - second.Y) <= first.Height / 2 && first.X < second.X);

    [OfflineFact]
    public void ThePaneAsksForTheServersAddressAlone() =>
        Case(nameof(ThePaneAsksForTheServersAddressAlone), app =>
        {
            OpenPane(app);
            var types = new[] { "ImapChoice", "JmapChoice", "NextcloudChoice", "DavChoice" }
                .Select(id => Shown(app, id) ?? throw new InvalidOperationException($"the picker draws no #{id}"))
                .ToList();
            Assert.True(types.Zip(types.Skip(1)).All(pair => Before(pair.First, pair.Second)),
                "the picker draws the account types in the core's order (manual_setup_kinds)");

            Assert.NotNull(Shown(app, "NextcloudServer"));
            foreach (var other in new[] { "DavLogin", "DavPassword", "DavCaldavUrl", "Username", "ImapHost", "JmapEmail", "ConnectButton" })
            {
                Assert.True(Shown(app, other) is null, $"the Nextcloud pane asks for the address alone, yet #{other} is drawn");
            }
            Assert.False(SignIn(app).Enabled, "the sign-in waits for an address");
            app.Agent.SetText(Address(app), "cloud.example.org");
            Assert.True(App.Eventually(() => SignIn(app).Enabled, TimeSpan.FromSeconds(5)), "an address enables the sign-in");
            app.Agent.SetText(Address(app), "   ");
            Assert.True(App.Eventually(() => !SignIn(app).Enabled, TimeSpan.FromSeconds(5)), "blanks are no address");
        });

    [OfflineFact]
    public void WhatIsNoAddressIsSaidWithoutAsking() =>
        Case(nameof(WhatIsNoAddressIsSaidWithoutAsking), app =>
        {
            OpenPane(app);
            // Plain HTTP to another machine: the core refuses it before any request.
            app.Agent.SetText(Address(app), "http://cloud.example.org");
            app.Agent.Invoke(SignIn(app));
            var said = Catalog.English("setup_nextcloud_address_invalid");
            Assert.True(App.Eventually(() => app.Agent.Find(Filter.Named(said) with { OnScreen = true }) is not null, TimeSpan.FromSeconds(10)),
                "the pane says why no sign-in started");
            Assert.Equal("http://cloud.example.org", app.Agent.Read(Address(app)).Value);
        });

    [OfflineFact]
    public void AnAddressThatAnswersAsNextcloudWaitsOnTheBrowserAndCancelKeepsTheAddress() =>
        Case(nameof(AnAddressThatAnswersAsNextcloudWaitsOnTheBrowserAndCancelKeepsTheAddress), app =>
        {
            using var server = new NextcloudStandIn();
            OpenPane(app);
            app.Agent.SetText(Address(app), server.Dav);
            app.Agent.Invoke(SignIn(app));
            Assert.True(App.Eventually(() => server.Count(NextcloudStandIn.Begin) == 1, TimeSpan.FromSeconds(15)),
                "an address that answers as Nextcloud starts its sign-in");
            Assert.Equal(1, server.Count(NextcloudStandIn.Status));
            var waiting = Catalog.English("nextcloud_sign_in_waiting");
            Assert.True(App.Eventually(() => Shown(app, "NextcloudAddressWaitingText")?.Name == waiting, TimeSpan.FromSeconds(10)),
                "rule 5: while the browser holds the sign-in the pane says to continue there");
            Assert.True(Shown(app, "NextcloudUnsurePanel") is null, "nothing is offered anyway for a server that answered");

            var cancel = Shown(app, "CancelButton");
            Assert.True(cancel is { Enabled: true }, "onboarding.md: the wait has a way out, drawn with it");
            app.Agent.Invoke(cancel!);
            Assert.True(App.Eventually(() => Shown(app, "NextcloudAddressWaitingText") is null, TimeSpan.FromSeconds(5)), "Cancel takes the wait away");
            Assert.True(App.Eventually(() => SignIn(app).Enabled, TimeSpan.FromSeconds(5)), "and leaves the pane, its sign-in ready to press again");
            Assert.Equal(server.Dav, app.Agent.Read(Address(app)).Value);
        });

    [OfflineFact]
    public void AnAddressThatShowsNothingIsOfferedTryAnywayWhileThePaneHoldsIt() =>
        Case(nameof(AnAddressThatShowsNothingIsOfferedTryAnywayWhileThePaneHoldsIt), app =>
        {
            using var server = new NextcloudStandIn();
            OpenPane(app);
            app.Agent.SetText(Address(app), server.Base);
            app.Agent.Invoke(SignIn(app));
            Assert.True(App.Eventually(() => Shown(app, "NextcloudTryAnywayButton") is not null, TimeSpan.FromSeconds(15)),
                "an address the core could not tell is Nextcloud is offered the sign-in anyway");
            Assert.Contains("127.0.0.1", Shown(app, "NextcloudUnsureText")?.Name ?? "", StringComparison.Ordinal);
            Assert.True(server.Count(NextcloudStandIn.WellKnown) > 0, "the core asked the address's .well-known");
            Assert.Equal(0, server.Count(NextcloudStandIn.Status));
            Assert.Equal(0, server.Count(NextcloudStandIn.Begin));

            // The offer is for the address asked about, and only while the field holds it.
            app.Agent.SetText(Address(app), server.Base + "/other");
            Assert.True(App.Eventually(() => Shown(app, "NextcloudTryAnywayButton") is null, TimeSpan.FromSeconds(5)),
                "another address takes the offer away");
            app.Agent.SetText(Address(app), server.Base);
            Assert.True(App.Eventually(() => Shown(app, "NextcloudTryAnywayButton") is not null, TimeSpan.FromSeconds(5)),
                "the address asked about brings it back");

            app.Agent.Invoke(Shown(app, "NextcloudTryAnywayButton")!);
            Assert.True(App.Eventually(() => server.Count(NextcloudStandIn.Begin) == 1, TimeSpan.FromSeconds(15)),
                "Try anyway starts the sign-in at the address typed");
            Assert.True(App.Eventually(() => Shown(app, "NextcloudAddressWaitingText") is not null, TimeSpan.FromSeconds(10)),
                "and waits on the browser");
            Assert.True(Shown(app, "NextcloudUnsurePanel") is null, "starting a sign-in answers the offer");

            app.Agent.Invoke(Shown(app, "CancelButton")!);
            Assert.True(App.Eventually(() => SignIn(app).Enabled, TimeSpan.FromSeconds(5)), "Cancel hands the pane back");
            Assert.Equal(server.Base, app.Agent.Read(Address(app)).Value);
        });
}
