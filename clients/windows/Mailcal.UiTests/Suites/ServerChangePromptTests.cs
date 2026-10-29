// The question a calendar paused for a server on another site asks (docs/account-autodetect.md
// rule 14), on both of the surfaces that ask it: the calendar's own line, whose Review opens a
// dialog, and the account's page in Settings, which asks inline because Settings is itself a dialog.
//
// WHY THE SHOWCASE. MAILCAL_SHOWCASE_SERVER_CHANGE (debug builds, read in the core) pauses the first
// account's calendar for a move from example.com to example.net, so everything from the
// connectivity snapshot to the question is real, and CI runs it. A showcase account has no servers
// to change, so the core refuses the allow, and that refusal being said is what proves Allow
// reached it. ServerChangeTests drives the allow that moves an account on the harness; the
// calendar's line is out of its reach, and this is where it is asserted.
//
// Neither key a person might press to get past the dialog allows anything: Escape is its close
// response and Enter its default, and both are "Not now". The twins are
// clients/apple/UITests/ServerChangeTests.swift and the server-change leg of
// scripts/dev/test-linux-ui.sh.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The showcase with the first account's calendar paused for a server on another site.</summary>
public sealed class ServerChangeShowcaseApp : ShowcaseApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_SHOWCASE_SERVER_CHANGE"] = "1" };
}

[Trait("Dataset", "showcase")]
public sealed class ServerChangePromptTests(ServerChangeShowcaseApp showcase) : IClassFixture<ServerChangeShowcaseApp>
{
    private static readonly string Paused = Catalog.English("settings_account_use_server_changed_calendar");
    private static readonly string ReviewLabel = Catalog.English("server_changed_review");
    private static readonly string BeforeLine = Catalog.English("server_changed_before").Replace("{site}", "example.com");
    private static readonly string NowLine = Catalog.English("server_changed_now").Replace("{site}", "example.net");

    private void Case(string name, Action<App, SetupFlow> body) =>
        showcase.Case(name, app => body(app, new SetupFlow(app)), app => app.CloseDialogs());

    /// <summary>Whether any element under <paramref name="root"/> is named <paramref name="text"/>,
    /// or with <paramref name="within"/> has a name containing it.</summary>
    private static bool Says(App app, Element root, string text, bool within = false) =>
        app.Agent.FindAll(new Filter { Under = root }).Any(e => within ? e.Name.Contains(text, StringComparison.Ordinal) : e.Name == text);

    private static string NameOf(App app, Element root, string id) =>
        app.Agent.Find(new Filter { Id = id, Under = root })?.Name ?? throw new InvalidOperationException($"no #{id}");

    /// <summary>The calendar's line for the paused calendar, with the calendar on screen.</summary>
    private static Element Line(App app)
    {
        app.Agent.Invoke(app.Agent.Find(Filter.ById("NavCalendar", "ListItem")) ?? throw new InvalidOperationException("no Calendar entry in the navigation pane"));
        return app.Agent.Wait(Filter.ById("CalendarOutageBanner") with { OnScreen = true }, TimeSpan.FromSeconds(30))
            ?? throw new InvalidOperationException("the calendar draws no line for its paused use. Is this a DEBUG build? MAILCAL_SHOWCASE_SERVER_CHANGE is compiled out of Release");
    }

    private static Element ReviewOn(App app, Element line) =>
        app.Agent.Find(new Filter { Id = "CalendarOutageAction", Under = line })
            ?? throw new InvalidOperationException("the paused line has no action button (#CalendarOutageAction)");

    /// <summary>Presses Review on the calendar's line and returns the dialog it opens, checked for
    /// both sites.</summary>
    private static Element OpenPrompt(App app)
    {
        var review = ReviewOn(app, Line(app));
        Assert.Equal(ReviewLabel, review.Name);
        app.Agent.Invoke(review);
        var prompt = app.Agent.Wait(Filter.ById("ServerChangePrompt"), TimeSpan.FromSeconds(20))
            ?? throw new InvalidOperationException("Review opened no question (#ServerChangePrompt)");
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        Assert.True(Says(app, prompt, Catalog.English("server_changed_title_calendar")), "the dialog is titled for the calendar");
        Assert.Equal(BeforeLine, NameOf(app, prompt, "ServerChangeBefore"));
        Assert.Equal(NowLine, NameOf(app, prompt, "ServerChangeNow"));
        return prompt;
    }

    /// <summary>Answers the open dialog with <paramref name="key"/> and asserts it closed with the
    /// calendar still paused.</summary>
    private static void AssertKeyLeavesItPaused(App app, ushort key, string name)
    {
        Input.BringToFront(app.Mailbox.Hwnd);
        Input.Press(key);
        Assert.True(app.Agent.WaitGone(Filter.ById("ServerChangePrompt"), TimeSpan.FromSeconds(20)), $"{name} answers the question");
        var line = Line(app);
        Assert.True(Says(app, line, Paused, within: true), $"and after {name} the calendar is still paused: {name} is Not now, never Allow");
        Assert.Equal(ReviewLabel, ReviewOn(app, line).Name);
    }

    [OfflineFact]
    public void TheCalendarSaysItIsPausedAndReviewAsksNamingBothSites() => Case(nameof(TheCalendarSaysItIsPausedAndReviewAsksNamingBothSites), (app, _) =>
    {
        Assert.True(Says(app, Line(app), Paused, within: true), "the calendar line says the calendar is paused for a new server");
        OpenPrompt(app);
        AssertKeyLeavesItPaused(app, Input.Escape, "Escape");
    });

    [OfflineFact]
    public void EnterIsNotNowTooSoAStrayReturnAllowsNothing() => Case(nameof(EnterIsNotNowTooSoAStrayReturnAllowsNothing), (app, _) =>
    {
        OpenPrompt(app);
        AssertKeyLeavesItPaused(app, Input.Enter, "Enter");
    });

    [OfflineFact]
    public void TheAccountPageAsksInlineAndARefusedAllowSaysSoAndKeepsAsking() => Case(nameof(TheAccountPageAsksInlineAndARefusedAllowSaysSoAndKeepsAsking), (app, settings) =>
    {
        var rows = settings.AccountRows();
        var dialog = settings.OpenAccount(rows[0].Name);
        Assert.True(Says(app, dialog, Paused), "the calendar switch on the account page says it is paused");
        app.Agent.Invoke(app.Agent.Find(new Filter { Id = "AccountUseServerChangeReview_Calendar", Under = dialog })
            ?? throw new InvalidOperationException("the account page offers no Review under the calendar (#AccountUseServerChangeReview_Calendar)"));
        // The question is a layout panel, which has no automation peer: it is found by its parts.
        var allow = app.Agent.Wait(new Filter { Id = "ServerChangeAllow", Type = "Button", Under = dialog }, TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("Review drew no question on the account page (#ServerChangeAllow)");
        Assert.Equal(BeforeLine, NameOf(app, dialog, "ServerChangeBefore"));
        Assert.Equal(NowLine, NameOf(app, dialog, "ServerChangeNow"));
        app.Agent.Invoke(allow);
        var refused = Catalog.English("server_changed_failed");
        Assert.True(App.Eventually(
            () => app.Agent.Find(new Filter { Id = "ServerChangeError", Under = dialog }) is { } error && Says(app, error, refused),
            TimeSpan.FromSeconds(10)),
            "a refused Allow says nothing was changed, where it was asked");
        Assert.True(app.Agent.Find(new Filter { Id = "ServerChangeAllow", Type = "Button", Under = dialog }) is not null, "and the question stays open to be answered again");
        Assert.True(Says(app, dialog, Paused), "and the calendar is still paused");
    });
}
