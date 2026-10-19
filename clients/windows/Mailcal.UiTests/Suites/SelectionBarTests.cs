// The mail actions bar across the window (docs/list-selection.md, rule 5): one row of buttons that
// stands whether or not anything is selected, so every button in it has one height and sits on the
// same two edges. It spans the folder pane too, with the two buttons that name no selection at its
// ends: New Mail at the head, where collapsing the pane cannot take it off screen, and Sync at the
// tail. Clear selection is the one button with a glyph and no label, and the default style centres
// its content rather than stretching it, so it once stood a few pixels short of the rest. Nothing
// headless sees a measured height.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class SelectionBarTests(ShowcaseApp showcase)
{
    // In the order the markup lays them out, named rather than swept off the container, so a button
    // that stops drawing fails here instead of leaving the comparison to its siblings.
    private static readonly string[] BarButtons =
    [
        Filter.ById("ComposeButton").Id!, Filter.ById("SelectionMarkRead").Id!, Filter.ById("SelectionFlag").Id!,
        Filter.ById("SelectionArchive").Id!, Filter.ById("SelectionDelete").Id!, Filter.ById("SelectionDeletePermanently").Id!,
        Filter.ById("SelectionSelectAll").Id!, Filter.ById("SelectionClear").Id!, Filter.ById("SelectionSync").Id!,
    ];

    // Physical pixels, slack for rounding rather than design: the labelled buttons agree exactly.
    private const double Tolerance = 1.5;

    private static Dictionary<string, Element> Buttons(App app) =>
        BarButtons.ToDictionary(id => id, id =>
            app.Agent.Wait(new Filter { Id = id, Type = "Button", Window = app.Mailbox.Hwnd }, App.Patience) is { Rendered: true } button
                ? app.Steady(button)
                : throw new InvalidOperationException($"the actions bar does not draw #{id}"));

    /// <summary>Whether a message row is selected, read off the list rather than off another button
    /// bound to the same flag: two controls sharing one broken binding agree perfectly.</summary>
    private static bool RowSelected(App app) => app.Rows().Any(row => app.Agent.Read(row).Selected == true);

    private static double Spread(IEnumerable<double> values) => values.Max() - values.Min();

    private static string Edges(Dictionary<string, Element> buttons) =>
        string.Join(" | ", buttons.Select(b => $"{b.Key}={b.Value.Y:0.#}..{b.Value.Y + b.Value.Height:0.#}"));

    [OfflineFact]
    public void EveryButtonInTheActionsBarIsTheSameHeight() => showcase.Case(nameof(EveryButtonInTheActionsBarIsTheSameHeight), app =>
    {
        var buttons = Buttons(app);
        Assert.True(Spread(buttons.Values.Select(b => b.Height)) <= Tolerance, $"the bar's buttons are one height, or the row reads as ragged. Edges: {Edges(buttons)}");
    });

    [OfflineFact]
    public void EveryButtonInTheActionsBarSitsOnTheSameTopAndBottomEdges() => showcase.Case(nameof(EveryButtonInTheActionsBarSitsOnTheSameTopAndBottomEdges), app =>
    {
        // Equal heights alone are also met by buttons stepping down the bar.
        var buttons = Buttons(app);
        Assert.True(Spread(buttons.Values.Select(b => b.Y)) <= Tolerance, $"the bar's buttons share a top edge. Edges: {Edges(buttons)}");
        Assert.True(Spread(buttons.Values.Select(b => b.Y + b.Height)) <= Tolerance, $"and a bottom edge. Edges: {Edges(buttons)}");
    });

    [OfflineFact]
    public void NewMailStandsAtTheHeadOfTheBarAndNeverFollowsTheSelection() => showcase.Case(nameof(NewMailStandsAtTheHeadOfTheBarAndNeverFollowsTheSelection), app =>
    {
        var buttons = Buttons(app);
        Assert.True(buttons["ComposeButton"].Enabled, "New Mail stays live whatever is selected: it writes a message, it acts on no selection");
        // Archive beside it follows the selection, or this proves nothing about the two differing.
        Assert.Equal(RowSelected(app), buttons["SelectionArchive"].Enabled);
        Assert.True(buttons["ComposeButton"].X + buttons["ComposeButton"].Width <= buttons["SelectionMarkRead"].X, "New Mail comes before the selection's own buttons");
    });

    [OfflineFact]
    public void NewMailSurvivesACollapsedFolderPane() => showcase.Case(nameof(NewMailSurvivesACollapsedFolderPane), app =>
    {
        // Why it is on this row at all: it used to head the folder pane, which collapses to a strip
        // of icons, and nothing else in the app writes a message.
        app.Agent.Invoke(app.Agent.Find(Filter.ById("PART_PaneToggleButton", "Button")) ?? throw new InvalidOperationException("no pane toggle in the caption"));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
        Assert.True(app.Agent.Find(Filter.ById("ComposeButton", "Button")) is { Rendered: true }, "New Mail stays on screen with the folder pane collapsed");
    }, app =>
    {
        // Back as the suite found it: the launch is shared.
        app.Agent.Invoke(app.Agent.Find(Filter.ById("PART_PaneToggleButton", "Button"))!);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
    });

    [OfflineFact]
    public void SyncStandsAtTheEndOfTheBarAndNeverFollowsTheSelection() => showcase.Case(nameof(SyncStandsAtTheEndOfTheBarAndNeverFollowsTheSelection), app =>
    {
        var buttons = Buttons(app);
        Assert.True(buttons["SelectionSync"].Enabled, "Sync stays live whatever is selected: it syncs the mailbox, not the selection");
        Assert.Equal(RowSelected(app), buttons["SelectionArchive"].Enabled);
        Assert.True(buttons["SelectionSync"].X >= buttons["SelectionClear"].X + buttons["SelectionClear"].Width, "Sync comes after the selection's own buttons");
    });
}
