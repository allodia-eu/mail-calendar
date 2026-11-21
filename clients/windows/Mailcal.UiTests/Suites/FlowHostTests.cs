// The flow host (Controls/FlowHost.cs; docs/ai.md, "The flow host"), through its debug-only preview:
// with MAILCAL_FLOW_PREVIEW=1, Settings → Advanced draws the host over two sample stages, one whose
// flow gives a step count and one whose flow gives none, its primary running, and under it a
// learning review fetching older sent mail. What is pinned is what the host puts on screen from a
// frame: the footer's order, a control drawn as its state says, and pips only for a count; and what
// the review draws while it fetches.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The showcase with the flow host's preview in Settings → Advanced.</summary>
public sealed class FlowPreviewApp : ShowcaseApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_FLOW_PREVIEW"] = "1" };
}

[Trait("Dataset", "showcase")]
public sealed class FlowHostTests(FlowPreviewApp showcase) : IClassFixture<FlowPreviewApp>
{
    /// <summary>Opens the preview and returns the dialog it is drawn in.</summary>
    private static Element Host(App app)
    {
        var dialog = new SetupFlow(app).OpenCategory("advanced");
        return app.Agent.Wait(new Filter { Id = "FlowPreviewTitle", Under = dialog }, TimeSpan.FromSeconds(10)) is null
            ? throw new InvalidOperationException("Settings → Advanced draws no flow preview with MAILCAL_FLOW_PREVIEW=1")
            : dialog;
    }

    private static Element? Part(App app, Element host, string id) => app.Agent.Find(new Filter { Id = "FlowPreview" + id, Under = host });

    [OfflineFact]
    public void ACountedStageDrawsItsPipsAndAFooterOfCloseThenThePrimary() =>
        showcase.Case(nameof(ACountedStageDrawsItsPipsAndAFooterOfCloseThenThePrimary), app =>
        {
            var host = Host(app);
            Assert.Equal("Sample flow", Part(app, host, "Title")?.Name);
            var steps = Part(app, host, "Steps") ?? throw new InvalidOperationException("a stage with a step count draws no pips");
            Assert.Equal("Step 1 of 3", steps.Name);
            Assert.True(Part(app, host, "Back") is null, "a stage without Back draws none");
            var close = Part(app, host, "Close") ?? throw new InvalidOperationException("the footer has no Close");
            var primary = Part(app, host, "Primary") ?? throw new InvalidOperationException("the footer has no primary");
            Assert.True(close.X < steps.X && steps.X < primary.X, "Close, then the pips, then the primary");
            Assert.True(primary.Enabled, "a primary the frame gives free can be pressed");
        }, app => app.CloseDialogs());

    [OfflineFact]
    public void AStageWithNoCountDrawsNoPipsAndARunningPrimaryShowsItsRing() =>
        showcase.Case(nameof(AStageWithNoCountDrawsNoPipsAndARunningPrimaryShowsItsRing), app =>
        {
            var host = Host(app);
            app.Agent.Invoke(Part(app, host, "Primary")!);
            Assert.True(App.Eventually(() => Part(app, host, "Back") is not null, TimeSpan.FromSeconds(5)), "the next stage has Back");
            // The footer is drawn afresh for the stage, and a control read before layout has no place.
            app.Agent.Settle();
            Assert.True(Part(app, host, "Steps") is null, "a flow with no count draws no pips");
            var back = Part(app, host, "Back")!;
            var primary = Part(app, host, "Primary")!;
            var close = Part(app, host, "Close")!;
            Assert.True(close.X < back.X && back.X < primary.X,
                $"Close, then Back, then the primary: Close at {close.X}, Back at {back.X}, the primary at {primary.X}");
            Assert.False(primary.Enabled, "a running primary cannot be pressed again");
            Assert.True(Part(app, host, "Primary_Running") is not null, "a running primary shows its ring");

            app.Agent.Invoke(back);
            Assert.True(App.Eventually(() => Part(app, host, "Steps") is not null, TimeSpan.FromSeconds(5)),
                "Back draws the stage the frame names, its pips with it");
        }, app => app.CloseDialogs());

    // docs/ai.md rule 11: a review fetching older sent mail draws the platform's spinner with its
    // line, as reading does, and one whose older mail could not be fetched says so.
    [OfflineFact]
    public void AReviewFetchingOlderMailShowsItsSpinnerAndSaysWhatCouldNotBeFetched() =>
        showcase.Case(nameof(AReviewFetchingOlderMailShowsItsSpinnerAndSaysWhatCouldNotBeFetched), app =>
        {
            // The preview's first stage runs nothing, so every spinner on the page is a review's: one
            // fetching, one reading after its fetch failed.
            var dialog = Host(app);
            Element? Under(string id) => app.Agent.Find(new Filter { Id = id, Under = dialog });
            Assert.Equal(Catalog.English("learn_fetching"), Under("WritingStyleFlowFetching")?.Name);
            Assert.Equal(Catalog.English("learn_reading"), Under("WritingStyleFlowReading")?.Name);
            var spinners = app.Agent.FindAll(new Filter { Type = "ProgressBar", Under = dialog }).Count;
            Assert.True(spinners == 2, $"fetching and reading each draw the platform's spinner; the page draws {spinners}");
            Assert.Equal(Catalog.English("learn_older_not_fetched"), Under("WritingStyleFlowOlderNotFetched")?.Name);
            Assert.True(app.Agent.FindAll(new Filter { Id = "WritingStyleFlowOlderNotFetched", Under = dialog }).Count == 1,
                "only the review whose fetch failed says so");
        }, app => app.CloseDialogs());
}
