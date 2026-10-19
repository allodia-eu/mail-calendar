// The background-sync hint lives inside the footer's status line and never takes a row of its own
// (docs/sync-progress.md). The bar belongs to a download the user is waiting on; this is a poll
// tick, an IDLE push, a boot catch-up, work nobody asked for, and given a row it would resize the
// list every few minutes. A real background pass is up for a fraction of a second, so it is staged
// (SyncStatusFlow.cs).

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The showcase with a background pass staged three folders into twelve. Neither round
/// nor equal: "0 of 0" reads like an unwired binding, and equal counts pass on either half.</summary>
public sealed class SyncHintApp : ShowcaseApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_FAKE_SYNC_HINT"] = $"{SyncStatusFlow.Account}:3/12" };
}

[Trait("Dataset", "showcase")]
public sealed class SyncHintTests(SyncHintApp showcase) : IClassFixture<SyncHintApp>
{
    [ShowcaseFact]
    public void ABackgroundSyncNamesItsAccountAndItsFolderCounts() => showcase.Case(nameof(ABackgroundSyncNamesItsAccountAndItsFolderCounts), app =>
        // The whole caption: the address proves the client resolved the id against its own
        // accounts, and the counts that both numbers travelled.
        Assert.Equal(
            Catalog.English("sync_hint_account").Replace("{account}", SyncStatusFlow.Account).Replace("{done}", "3").Replace("{total}", "12"),
            SyncStatusFlow.Caption(app).Name));

    [ShowcaseFact]
    public void TheHintSharesTheFooterRowAndNeverOpensOneOfItsOwn() => showcase.Case(nameof(TheHintSharesTheFooterRowAndNeverOpensOneOfItsOwn), app =>
    {
        SyncStatusFlow.AssertInStatusLine(app);
        // The footer reads count, then what is arriving, then whether we are connected; and the
        // status stays inside the list's column. A status line that measured the hint against
        // infinite width once pushed the status past the edge, present in the tree and empty on
        // screen; the hint holds the elastic column, so it is the thing that trims.
        var hint = SyncStatusFlow.Caption(app);
        var status = app.Agent.Find(Filter.ById("ConnectionStatus") with { OnScreen = true })!;
        var list = app.Agent.Find(Filter.ById("RowsList"))!;
        Assert.True(hint.X + hint.Width <= status.X, $"the hint (right edge {hint.X + hint.Width}) sits before the connection status (left edge {status.X})");
        Assert.True(status.X + status.Width <= list.X + list.Width,
            $"the connection status (right edge {status.X + status.Width}) stays inside the list's column (right edge {list.X + list.Width})");
    });

    [ShowcaseFact]
    public void ABackgroundSyncRaisesNoBarSoTheMessageListDoesNotMove() => showcase.Case(nameof(ABackgroundSyncRaisesNoBarSoTheMessageListDoesNotMove), app =>
    {
        SyncStatusFlow.Caption(app);
        // Nothing stages a download here, so any progress bar is a background pass promoted to the
        // download bar, which once resized the list for work the user never started.
        Assert.Empty(app.Agent.FindAll(new Filter { Type = "ProgressBar", Window = app.Mailbox.Hwnd, OnScreen = true }));
    });
}
