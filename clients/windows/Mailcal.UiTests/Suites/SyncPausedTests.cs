// The status line's third state: an account whose server asked us to wait (docs/sync-progress.md).
// A provider answered and declined the work for a while (Gmail's per-minute quota, a JMAP
// concurrency ceiling): nothing is arriving and nothing is wrong, which neither the bar nor the
// hint can say, and which used to read as an outage.
//
// The line is a short label and the wait lives in its tooltip and spoken name, two properties bound
// apart. A client that bound the label and forgot the rest draws a plausible "Sync paused" that
// never says when, and no headless check notices.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The showcase with the primary account staged as asked to wait five minutes.</summary>
public sealed class SyncPausedApp : ShowcaseApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_FAKE_SYNC_PAUSED"] = $"{SyncStatusFlow.Account}:5" };
}

[Trait("Dataset", "showcase")]
public sealed class SyncPausedTests(SyncPausedApp showcase) : IClassFixture<SyncPausedApp>
{
    [ShowcaseFact]
    public void APausedAccountSaysSoInTheStatusLineAndSaysWhenInItsTooltip() => showcase.Case(nameof(APausedAccountSaysSoInTheStatusLineAndSaysWhenInItsTooltip), app =>
        // The spoken name is the whole sentence, bound explicitly: a screen reader given the text
        // alone would hear "Sync paused" and never the wait. It is the tooltip's string too, so
        // this checks the tooltip without hovering.
        Assert.Equal(
            Catalog.English("sync_paused_detail").Replace("{account}", SyncStatusFlow.Account).Replace("{count}", "5"),
            SyncStatusFlow.Caption(app).Name));

    [ShowcaseFact]
    public void ThePauseKeepsTheStatusLineOffItsOwnRowExactlyAsTheHintDoes() => showcase.Case(nameof(ThePauseKeepsTheStatusLineOffItsOwnRowExactlyAsTheHintDoes), app =>
        SyncStatusFlow.AssertInStatusLine(app));
}
