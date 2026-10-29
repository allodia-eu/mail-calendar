// The background hint's second phase: warming message bodies after a sync, so the mailbox reads
// offline (docs/sync-progress.md). On a first sync it is the longer half, and it has no total, so
// the caption says how far it has got and no more. What differs from the folder phase is only which
// string the client picks, which is exactly the bug: an unread field reads as false, the folder
// branch is taken, and the footer says "0 of 0 folders" for minutes of real downloading.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The showcase with a body warm staged at 2,022 messages: no denominator, which is what
/// selects the body phase, and over a thousand, so the caption needs the client's own thousands
/// separator, a comma in English.</summary>
public sealed class SyncHintBodiesApp : ShowcaseApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_FAKE_SYNC_HINT"] = $"{SyncStatusFlow.Account}:2022" };
}

[Trait("Dataset", "showcase")]
public sealed class SyncHintBodiesTests(SyncHintBodiesApp showcase) : IClassFixture<SyncHintBodiesApp>
{
    [OfflineFact]
    public void ABodyWarmReportsHowManyMessagesAreDownSoFarNotAFolderCount() => showcase.Case(nameof(ABodyWarmReportsHowManyMessagesAreDownSoFarNotAFolderCount), app =>
        Assert.Equal(
            Catalog.English("sync_hint_bodies").Replace("{account}", SyncStatusFlow.Account).Replace("{done}", "2,022"),
            SyncStatusFlow.Caption(app).Name));
}
