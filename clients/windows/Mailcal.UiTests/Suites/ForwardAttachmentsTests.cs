// Forwarding a message opens the composer holding the files the original carries, as ordinary
// attachments that can be taken off again (docs/sending.md, "What a forward carries").
//
// WHY HERE. Every rule below lives in ComposerView.xaml.cs, which links WinUI. The attachment list
// is not bound in XAML, so its rows exist only once its ItemsSource is assigned again, and a seed
// that filled the files and skipped that would leave a composer the user can neither see the files
// in nor remove one from, while the message still went out carrying them.
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
    private static readonly string StagingRoot = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Allodia", "MailCalendar", "composer-files");

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

    /// <summary>The rows in the composer's own list: the reading pane's chip for the same file is
    /// still in the tree behind it.</summary>
    private static IReadOnlyList<Element> Attachments(App app)
    {
        var list = app.Agent.Find(Filter.ById("AttachmentList")) ?? throw new InvalidOperationException("the composer draws no AttachmentList");
        return app.Agent.FindAll(new Filter { Type = "ListItem", Under = list });
    }

    private static Element TheStagedFile(App app)
    {
        var rows = Attachments(app);
        return rows.Count == 1
            ? rows[0]
            : throw new InvalidOperationException($"the forward composer holds {rows.Count} attachment row(s), not the one file the original carries");
    }

    private static Element Remove(App app) =>
        app.Agent.Find(Filter.ById("RemoveAttachmentButton", "Button")) ?? throw new InvalidOperationException("the composer offers no Remove button");

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
        var rows = Attachments(app);
        Assert.True(rows.Count == 1,
            $"the forward composer shows {rows.Count} attachment row(s) for a message carrying one file: an empty list claims the original had nothing attached");
        Assert.True(rows[0].Name == FileName,
            $"the row reads '{rows[0].Name}': it carries the name the sender put on the part, not the path the core staged the bytes to");
    });

    [HarnessFact]
    public void TheFileIsRemovableLikeAnythingElseTheUserAttached() => Case(nameof(TheFileIsRemovableLikeAnythingElseTheUserAttached), app =>
    {
        OpenForward(app);
        Assert.False(Remove(app).Enabled, "Remove is live before any row is selected, so pressing it says nothing about which file it meant");
        app.Agent.Select(TheStagedFile(app));
        Assert.True(App.Eventually(() => app.Agent.Read(Remove(app)).Element.Enabled, TimeSpan.FromSeconds(3)),
            "selecting the staged file did not enable Remove, so a forward imposes its files rather than proposing them");
        app.Agent.Invoke(Remove(app));
        Assert.True(App.Eventually(() => Attachments(app).Count == 0, TimeSpan.FromSeconds(3)),
            "the row survived Remove: a staged file that cannot be taken off goes out whether or not the sender meant to pass it on");
    });

    [HarnessFact]
    public void AnUntouchedForwardIsNotADraftSoDiscardingItAsksNothing() => Case(nameof(AnUntouchedForwardIsNotADraftSoDiscardingItAsksNothing), app =>
    {
        OpenForward(app);
        // Discard asks only when something would be lost (docs/drafts.md, "Leaving a composer"); the
        // forward saved nothing, so the question turns on the attachment baseline alone.
        Assert.False(DiscardAsks(app),
            "a forward nobody typed into was treated as written in: the composer measures the attachment count against zero rather than against what it opened with");
        Assert.True(app.Agent.WaitGone(Filter.ById("SendButton"), TimeSpan.FromSeconds(10)), "the composer stayed up after Discard");
    });

    [HarnessFact]
    public void TakingOneOffIsADecisionAboutWhatGoesOutAndDiscardAsks() => Case(nameof(TakingOneOffIsADecisionAboutWhatGoesOutAndDiscardAsks), app =>
    {
        OpenForward(app);
        app.Agent.Select(TheStagedFile(app));
        app.Agent.Invoke(Remove(app));
        App.Eventually(() => Attachments(app).Count == 0, TimeSpan.FromSeconds(3));
        Assert.True(DiscardAsks(app),
            "removing a forwarded file and pressing Discard threw that decision away unasked: the count changed, which is what the question is there to notice");
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
