// The download bar has its own row between the message list and the footer, never above the list
// (docs/sync-progress.md). Above it, every appearance resized the list and moved each row under the
// pointer; below it, the list's top cannot move.
//
// A real pass is up for a fraction of a second, so it is staged (SyncStatusFlow.cs). One launch is
// one state, so this proves the geometry rather than watching the bar come and go, which is enough:
// a bar that begins below the list cannot move what is above it.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The showcase with a download staged at 1,200 of 3,387: neither round, and both with a
/// thousands separator, so the caption cannot pass on one count alone.</summary>
public sealed class SyncProgressApp : ShowcaseApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_FAKE_SYNC_PROGRESS"] = "1200/3387" };
}

[Trait("Dataset", "showcase")]
public sealed class SyncProgressPlacementTests(SyncProgressApp showcase) : IClassFixture<SyncProgressApp>
{
    /// <summary>The staged bar, and the only one: a second would make every measurement a guess.</summary>
    private static Element Bar(App app)
    {
        var bar = app.Agent.Wait(new Filter { Type = "ProgressBar", Window = app.Mailbox.Hwnd, OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException(
                "no progress bar within 30 s: the staged download never reached the window (a Release build compiles the switch out), or SyncProgressVisible is not bound");
        var all = app.Agent.FindAll(new Filter { Type = "ProgressBar", Window = app.Mailbox.Hwnd, OnScreen = true });
        return all.Count == 1 ? bar : throw new InvalidOperationException($"expected one progress bar on the mail list, found {all.Count}");
    }

    private static Element Drawn(App app, string id) =>
        app.Agent.Find(Filter.ById(id) with { Window = app.Mailbox.Hwnd }) is { Rendered: true } element
            ? element
            : throw new InvalidOperationException($"#{id} is not drawn");

    [OfflineFact]
    public void AReportedDownloadRaisesTheBarCarryingTheCountsTheCoreGaveIt() => showcase.Case(nameof(AReportedDownloadRaisesTheBarCarryingTheCountsTheCoreGaveIt), app =>
    {
        Bar(app);
        // The caption proves the numbers travelled, not only the visibility: a bar drawn from a
        // hardcoded Visible passes every geometric case below and fails this one.
        Assert.True(app.Agent.Find(new Filter { Type = "Text", Window = app.Mailbox.Hwnd, NameLike = "Downloading 1,200 of 3,387*" }) is not null,
            "the caption beside the bar reads the counts the core reported");
    });

    [OfflineFact]
    public void TheBarSitsBelowTheMessageListSoABackgroundSyncCannotMoveARow() => showcase.Case(nameof(TheBarSitsBelowTheMessageListSoABackgroundSyncCannotMoveARow), app =>
    {
        var list = Drawn(app, "RowsList");
        var bar = Bar(app);
        Assert.True(bar.Y >= list.Y + list.Height, $"the bar's top ({bar.Y}) is at or below the list's bottom ({list.Y + list.Height})");
    });

    [OfflineFact]
    public void AndAboveTheFooterWhichIsStillTheBottomMostRow() => showcase.Case(nameof(AndAboveTheFooterWhichIsStillTheBottomMostRow), app =>
    {
        // Two rows sharing one grid row draw over each other, which the XAML compiler accepts.
        var bar = Bar(app);
        var status = Drawn(app, "ConnectionStatus");
        Assert.True(status.Y >= bar.Y + bar.Height, $"the connection status starts at {status.Y}, above the bar's bottom ({bar.Y + bar.Height})");
    });

    [OfflineFact]
    public void NoMessageRowIsLeftUnderneathTheBar() => showcase.Case(nameof(NoMessageRowIsLeftUnderneathTheBar), app =>
    {
        // The list gives the space up rather than drawing beneath the bar, which would pass every
        // comparison above while hiding mail.
        var bar = Bar(app);
        var rows = app.Rows().Where(r => r.Rendered).ToList();
        Assert.NotEmpty(rows);
        foreach (var row in rows)
        {
            Assert.True(row.Y < bar.Y, $"a message row starts at {row.Y}, at or below the bar's top ({bar.Y})");
        }
    });
}
