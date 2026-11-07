// The two reauth notices on Windows (docs/notices.md, docs/provider-oauth.md rule 11): the mail
// screen's bar holds the mail one, Blocked, with Sign in again; the calendar screen's holds the
// calendar's; and the account they are about says "2 issues" in the folder pane.
//
// MAILCAL_SHOWCASE_REAUTH (debug builds, read in the core) raises both on the showcase's first
// account, so everything from the connectivity snapshot to the bar is real and CI runs it. The
// twin is clients/apple/UITests/NoticeTests.swift.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The showcase with both reauth notices raised on its first account.</summary>
public sealed class ReauthShowcaseApp : ShowcaseApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_SHOWCASE_REAUTH"] = "1" };
}

[Trait("Dataset", "showcase")]
public sealed class ReauthNoticeTests(ReauthShowcaseApp showcase) : IClassFixture<ReauthShowcaseApp>
{
    private static readonly string[] Mail = Catalog.English("notice_mail_reauth").Split("{address}");
    private static readonly string[] Calendar = Catalog.English("notice_calendar_reauth").Split("{address}");

    /// <summary>The bar's sentence of the form <paramref name="words"/>, waited for, and the
    /// address it names.</summary>
    private static (Element Bar, string Address) Bar(App app, string[] words)
    {
        Element? bar = null;
        string? sentence = null;
        App.Eventually(() =>
        {
            bar = app.NoticeBar();
            sentence = bar is null ? null : app.Agent.FindAll(new Filter { Under = bar })
                .Select(e => e.Name)
                .FirstOrDefault(name => name.StartsWith(words[0], StringComparison.Ordinal) && name.EndsWith(words[1], StringComparison.Ordinal));
            return sentence is not null;
        }, TimeSpan.FromSeconds(30));
        if (bar is null || sentence is null)
        {
            throw new InvalidOperationException(
                $"no bar says \"{words[0]}…\". Is this a DEBUG build? MAILCAL_SHOWCASE_REAUTH is compiled out of Release");
        }
        return (bar, sentence[words[0].Length..^words[1].Length]);
    }

    [OfflineFact]
    public void TheMailScreenShowsMailsNoticeBlockedWithSignInAgain() => showcase.Case(nameof(TheMailScreenShowsMailsNoticeBlockedWithSignInAgain), app =>
    {
        app.RowsList();
        var (bar, _) = Bar(app, Mail);
        // Blocked is drawn as the InfoBar's Error severity, whose own icon names it.
        Assert.True(app.Agent.Find(new Filter { Id = "StandardIcon", Under = bar }) is { Name: "Error icon" },
            "a grant that cannot send or change mail is Blocked, drawn as an error");
        Assert.Equal(Catalog.English("notice_action_sign_in_again"), app.NoticeAction(bar)?.Name);
        Assert.True(app.NoticeClose(bar) is null, "a grant too narrow to send is a state, and cannot be closed");
    });

    [OfflineFact]
    public void TheCalendarScreenShowsTheCalendarsNotice() => showcase.Case(nameof(TheCalendarScreenShowsTheCalendarsNotice), app =>
    {
        app.Agent.Invoke(app.Agent.Find(Filter.ById("NavCalendar", "ListItem"))
            ?? throw new InvalidOperationException("no Calendar entry in the navigation pane"));
        var (bar, _) = Bar(app, Calendar);
        Assert.Equal(Catalog.English("notice_action_sign_in_again"), app.NoticeAction(bar)?.Name);
    });

    [OfflineFact]
    public void TheAccountSaysTwoIssuesInTheFolderPane() => showcase.Case(nameof(TheAccountSaysTwoIssuesInTheFolderPane), app =>
    {
        app.RowsList();
        var (_, address) = Bar(app, Mail);
        var twoIssues = Catalog.English("notices_account_count").Replace("{count}", "2");
        Assert.True(App.Eventually(() => app.AccountMark(address) == twoIssues, TimeSpan.FromSeconds(10)),
            $"the account both notices are about reads \"{twoIssues}\" beneath its address, and it reads \"{app.AccountMark(address)}\"");
    });
}
