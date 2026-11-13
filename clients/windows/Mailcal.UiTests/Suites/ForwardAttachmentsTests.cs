// Forwarding a message opens the composer holding the files the original carries, as ordinary
// attachments that can be taken off again (docs/sending.md, "What a forward carries").
//
// WHY HERE. The core holds a composer's files and says whether they changed, but the tiles are
// ComposerView's, which links WinUI: a composer that never drew the core's list would leave a file
// the user can neither see nor remove, while the message still went out carrying it.
//
// WHY THE HARNESS. Staging decodes the original's parts out of the raw source the engine fetched,
// the round trip the showcase engine does not make. 04-attachment.eml carries one text/csv part
// named by its sender, which makes the name assertion one about the sender's name, not a path.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class ForwardAttachmentsTests(HarnessApp harness)
{
    private const string Subject = "Message with a text attachment";

    // The name the sender put on the part. The staged copy is not called this (the core makes it
    // unique, so two composers cannot write over each other), which is the point of asserting it.
    private const string FileName = "report.csv";

    // Where the core stages a composer's files, one folder per composer (Services/ComposerStaging.cs).
    private static readonly string StagingRoot = Path.Combine(DataDir.Root, "composer-files");

    private void Case(string name, Action<App> body) => harness.Case(name, body, Discard);

    /// <summary>Puts the composer away by its Discard, answering the question when it asks.</summary>
    private static void Discard(App app)
    {
        if (app.Agent.Find(Filter.ById("PrimaryButton", "Button") with { OnScreen = true }) is { } discard)
        {
            app.Agent.Invoke(discard);
        }
        app.CloseComposer();
        app.CloseDialogs();
    }

    private static IReadOnlyList<string> StagingFolders() =>
        Directory.Exists(StagingRoot) ? Directory.GetDirectories(StagingRoot) : [];

    /// <summary>The forward composer for the seeded message. It opens after staging, never before
    /// (docs/sending.md), so the wait covers the file being decoded and written too.</summary>
    private static void OpenForward(App app)
    {
        app.OpenMessage(Subject);
        app.Agent.Invoke(app.Agent.Wait(Filter.ById("ReadingForward") with { OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException("the reading pane never drew its Forward button"));
        if (app.Agent.Wait(Filter.ById("SendButton"), App.Patience) is null)
        {
            throw new InvalidOperationException("no composer within 30 s of pressing Forward");
        }
    }

    /// <summary>The tiles in the composer's own file grid: the reading pane's chip for the same
    /// file is still in the tree behind it. The grid is not drawn while it holds nothing.</summary>
    private static IReadOnlyList<Element> Attachments(App app) =>
        app.Agent.Find(Filter.ById("ComposerFiles")) is { } grid
            ? app.Agent.FindAll(new Filter { Id = "ComposerFileTile", Type = "ListItem", Under = grid })
            : [];

    private static Element TheStagedFile(App app)
    {
        var tiles = Attachments(app);
        return tiles.Count == 1
            ? tiles[0]
            : throw new InvalidOperationException($"the forward composer holds {tiles.Count} file tile(s), not the one file the original carries");
    }

    /// <summary>The Remove control on <paramref name="tile"/> itself.</summary>
    private static Element Remove(App app, Element tile) =>
        app.Agent.Find(new Filter { Id = "ComposerFileRemove", Type = "Button", Under = tile })
            ?? throw new InvalidOperationException("the file's tile offers no Remove");

    /// <summary>Presses Discard and returns whether it asked first.</summary>
    private static bool DiscardAsks(App app)
    {
        app.Agent.Invoke(app.Agent.Find(Filter.ById("DiscardButton", "Button")) ?? throw new InvalidOperationException("the composer has no Discard"));
        app.Agent.Settle(floor: TimeSpan.FromMilliseconds(300), cap: TimeSpan.FromMilliseconds(1500));
        return app.Agent.Find(Filter.ById("CloseButton", "Button") with { OnScreen = true }) is not null;
    }

    [HarnessFact]
    public void AForwardOpensHoldingTheOriginalsFileUnderTheNameItsSenderGaveIt() => Case(nameof(AForwardOpensHoldingTheOriginalsFileUnderTheNameItsSenderGaveIt), app =>
    {
        OpenForward(app);
        var tiles = Attachments(app);
        Assert.True(tiles.Count == 1,
            $"the forward composer shows {tiles.Count} file tile(s) for a message carrying one file: an empty grid claims the original had nothing attached");
        Assert.True(tiles[0].Name == FileName,
            $"the tile reads '{tiles[0].Name}': it carries the name the sender put on the part, not the path the core staged the bytes to");
    });

    [HarnessFact]
    public void TheFileIsRemovableLikeAnythingElseTheUserAttached() => Case(nameof(TheFileIsRemovableLikeAnythingElseTheUserAttached), app =>
    {
        OpenForward(app);
        var remove = Remove(app, TheStagedFile(app));
        Assert.True(remove.Enabled, "the forwarded file's Remove is held, so a forward imposes its files rather than proposing them");
        app.Agent.Invoke(remove);
        Assert.True(App.Eventually(() => Attachments(app).Count == 0, TimeSpan.FromSeconds(3)),
            "the tile survived Remove: a staged file that cannot be taken off goes out whether or not the sender meant to pass it on");
    });

    [HarnessFact]
    public void AnUntouchedForwardIsNotADraftSoDiscardingItAsksNothing() => Case(nameof(AnUntouchedForwardIsNotADraftSoDiscardingItAsksNothing), app =>
    {
        OpenForward(app);
        // Discard asks only when something would be lost (docs/drafts.md, "Leaving a composer"); the
        // forward saved nothing, so the question turns on whether its files changed alone.
        Assert.False(DiscardAsks(app),
            "a forward nobody typed into was treated as written in: the files it opened holding counted as the user's own");
        Assert.True(app.Agent.WaitGone(Filter.ById("SendButton"), TimeSpan.FromSeconds(10)), "the composer stayed up after Discard");
    });

    [HarnessFact]
    public void TakingOneOffIsADecisionAboutWhatGoesOutAndDiscardAsks() => Case(nameof(TakingOneOffIsADecisionAboutWhatGoesOutAndDiscardAsks), app =>
    {
        OpenForward(app);
        app.Agent.Invoke(Remove(app, TheStagedFile(app)));
        App.Eventually(() => Attachments(app).Count == 0, TimeSpan.FromSeconds(3));
        Assert.True(DiscardAsks(app),
            "removing a forwarded file and pressing Discard threw that decision away unasked: the files changed, which is what the question is there to notice");
    });

    [HarnessFact]
    public void ADiscardedForwardLeavesNoCopyOfItsFileOnDisk() => Case(nameof(ADiscardedForwardLeavesNoCopyOfItsFileOnDisk), app =>
    {
        // The staged copy is the original's bytes, decoded: left behind, it is the attachment lying in
        // the user's profile for as long as the app runs.
        var before = StagingFolders();
        OpenForward(app);
        var staged = StagingFolders().Except(before).ToList();
        if (staged.Count != 1)
        {
            throw new InvalidOperationException($"the forward staged into {staged.Count} new folder(s) under {StagingRoot}, not its own one; this would watch the wrong place");
        }
        app.CloseComposer();
        Assert.True(App.Eventually(() => !Directory.Exists(staged[0]), TimeSpan.FromSeconds(10)),
            $"the discarded forward's staged copy is still at {staged[0]}: nothing will send or save it now");
    });
}
