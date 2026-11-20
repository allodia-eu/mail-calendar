// A preview of the flow host (Controls/FlowHost.cs) for the UI suite, debug builds only: with
// MAILCAL_FLOW_PREVIEW=1 at launch, Settings → Advanced draws the host over two sample stages in
// place of its own content, and under it the learning flow's review as it fetches older sent mail
// and as it reads once that fetch failed, states the showcase and the harness pass through too
// quickly to read, or never reach. Mailcal.Tests cannot draw a window, so this is how a suite sees
// what each puts on screen. The host's words are samples and are not in the catalog; nothing here
// is compiled into a release build.

#if DEBUG
using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private static readonly bool FlowPreviewOn =
        Environment.GetEnvironmentVariable("MAILCAL_FLOW_PREVIEW")?.Trim() == "1";

    private static readonly ControlState Ready = new(true, false);

    /// <summary>The preview, or null unless the launch asked for it.</summary>
    private UIElement? FlowPreview()
    {
        if (!FlowPreviewOn)
        {
            return null;
        }
        var host = new FlowHost("FlowPreview", 420);
        var counted = new TextBlock { Text = "A stage whose flow gives a step count.", TextWrapping = TextWrapping.Wrap };
        var open = new TextBlock { Text = "A stage whose flow gives none, its primary running.", TextWrapping = TextWrapping.Wrap };
        void Counted(int page) => host.Draw(
            new FlowFrame(
                "Sample flow",
                new FlowAction("Cancel", Ready, () => ShowCategory("general")),
                Primary: new FlowAction("Next", Ready, Open),
                Steps: new FlowSteps(3, page, $"Step {page + 1} of 3", Counted)),
            counted);
        void Open() => host.Draw(
            new FlowFrame(
                "Sample flow",
                new FlowAction("Cancel", Ready, () => ShowCategory("general")),
                Back: new FlowAction("Back", Ready, () => Counted(0)),
                Primary: new FlowAction("Learn from these messages", new ControlState(false, true), () => { })),
            open);
        Counted(0);

        // The review as the core sends it while it fetches older sent mail, and as it reads once that
        // fetch has failed.
        var preview = new StackPanel { Spacing = 24 };
        preview.Children.Add(host.Root);
        StyleLearningStage.Review Review(bool olderNotFetched, StyleLearningReview state) => new(
            "eva@example.com", new StyleLearnRange.LastTwelveMonths(), new StyleSentReach.Since("2026-08-20"),
            olderNotFetched, state, null, null, KeywordKept: true);
        preview.Children.Add(ReviewStage(Review(false, new StyleLearningReview.Fetching())));
        preview.Children.Add(ReviewStage(Review(true, new StyleLearningReview.Reading())));
        return preview;
    }
}
#endif
