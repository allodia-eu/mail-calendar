// The window's one notice bar (docs/notices.md): the first notice that belongs to the screen on
// show, its action, its close button where the notice can be closed, and the mark the account's row
// in the folder pane carries for it, readable rather than a tooltip.
//
// WHY THESE STAGINGS. The showcase pauses its first account's calendar for a server on another site
// (MAILCAL_SHOWCASE_SERVER_CHANGE), which the core raises as a calendar notice with Review: a notice
// that belongs to one screen and to one account, with no server, so CI runs it;
// ServerChangePromptTests presses its Review. A notice that can be closed is about something that
// already happened, and the one a harness launch can be made to raise is a stored account the core
// could not read: one is planted in the harness namespace's Credential Manager entries before the
// launch and taken out again after it.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Trait("Dataset", "showcase")]
public sealed class NoticeBarTests(ServerChangeShowcaseApp showcase) : IClassFixture<ServerChangeShowcaseApp>
{
    private static readonly string[] Outage = Catalog.English("notice_calendar_outage").Split("{address}");
    private static readonly string Detail = Catalog.English("notice_detail_server_changed_calendar");

    /// <summary>The calendar's bar, with the calendar on screen, and the sentence it says.</summary>
    private static (Element Bar, string Sentence) CalendarBar(App app)
    {
        app.Agent.Invoke(app.Agent.Find(Filter.ById("NavCalendar", "ListItem"))
            ?? throw new InvalidOperationException("no Calendar entry in the navigation pane"));
        string? sentence = null;
        Element? bar = null;
        App.Eventually(() =>
        {
            bar = app.NoticeBar();
            sentence = bar is null ? null : app.Agent.FindAll(new Filter { Under = bar })
                .Select(e => e.Name)
                .FirstOrDefault(name => name.StartsWith(Outage[0], StringComparison.Ordinal) && name.EndsWith(Outage[1], StringComparison.Ordinal));
            return sentence is not null;
        }, TimeSpan.FromSeconds(30));
        return (bar ?? throw new InvalidOperationException("the calendar draws no notice bar. Is this a DEBUG build? MAILCAL_SHOWCASE_SERVER_CHANGE is compiled out of Release"),
            sentence ?? throw new InvalidOperationException("the calendar's bar does not say its calendar is not connected"));
    }

    private static string AddressIn(string sentence) => sentence[Outage[0].Length..^Outage[1].Length];

    [OfflineFact]
    public void TheCalendarDrawsItsNoticeWithWhyAndReviewAndNoClose() => showcase.Case(nameof(TheCalendarDrawsItsNoticeWithWhyAndReviewAndNoClose), app =>
    {
        var (bar, _) = CalendarBar(app);
        Assert.True(app.Says(bar, Detail), "beneath the sentence, why: the calendar is paused for a new server, and the week is the last synced");
        var action = app.NoticeAction(bar);
        Assert.True(action is not null, "the notice offers its one action");
        Assert.Equal(Catalog.English("notice_action_review"), action!.Name);
        Assert.True(app.NoticeClose(bar) is null, "a state that holds cannot be closed: it goes when the state does");
        Assert.True(app.Agent.Find(new Filter { Id = "NoticeMore", Under = bar, OnScreen = true }) is null,
            "with one notice in force there is nothing more to list");
    });

    [OfflineFact]
    public void TheAccountRowSaysItInWordsOnEveryScreen() => showcase.Case(nameof(TheAccountRowSaysItInWordsOnEveryScreen), app =>
    {
        var (_, sentence) = CalendarBar(app);
        var address = AddressIn(sentence);
        app.RowsList();
        Assert.True(App.Eventually(() => app.NoticeBar() is null, TimeSpan.FromSeconds(10)),
            "the calendar's notice belongs to the calendar, so the mail has no bar");
        Assert.True(App.Eventually(() => app.AccountMark(address) == Catalog.English("notice_mark_calendar_paused"), TimeSpan.FromSeconds(10)),
            "and the account's row says it in a few words beneath the address, readable rather than a tooltip");
    });
}

/// <summary>The app on the harness store with a stored account the core cannot read beside it.</summary>
public sealed class SkippedAccountHarnessApp : HarnessApp
{
    /// <summary>The id the unreadable account is stored under, and its config: no TOML at all.</summary>
    internal const string Unreadable = "ui-suite-unreadable";

    /// <summary>The harness namespace the stalwart launch keeps its accounts in.</summary>
    internal const string Namespace = "dev";

    private protected override App Launch()
    {
        StoredAccounts.Plant(Namespace, Unreadable, "this is not an account config");
        return base.Launch();
    }
}

[Trait("Dataset", "harness")]
public sealed class NoticeCloseTests(SkippedAccountHarnessApp harness) : IClassFixture<SkippedAccountHarnessApp>
{
    [HarnessFact]
    public void AnAccountLeftOutAtLaunchIsSaidAndCanBeClosed() => harness.Case(nameof(AnAccountLeftOutAtLaunchIsSaidAndCanBeClosed), app =>
    {
        app.RowsList();
        var sentence = App.NoticeWords("notice_accounts_skipped");
        var bar = app.WaitNotice(sentence, TimeSpan.FromSeconds(30))
            ?? throw new InvalidOperationException("a stored account the core could not read raised no notice on the mail");
        Assert.True(app.NoticeAction(bar) is null, "there is nothing to run for an account nobody can read");
        var close = app.NoticeClose(bar);
        Assert.True(close is not null, "a notice about something that already happened can be closed");
        app.Agent.Invoke(close!);
        Assert.True(App.Eventually(() => app.NoticeBar() is not { } shown || !app.Says(shown, sentence), TimeSpan.FromSeconds(10)),
            "closing it takes it away");
        // The core remembers the dismissal until the fact goes, so a later snapshot does not bring it back.
        app.Agent.Invoke(app.Agent.Find(Filter.ById("SelectionSync", "Button"))!);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), cap: TimeSpan.FromSeconds(5));
        Assert.True(app.NoticeBar() is not { } again || !app.Says(again, sentence), "and a refresh does not raise it again");
    }, _ => StoredAccounts.Remove(SkippedAccountHarnessApp.Namespace, SkippedAccountHarnessApp.Unreadable));
}
