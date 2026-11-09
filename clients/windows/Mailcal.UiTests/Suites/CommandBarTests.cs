// The mail command bar under the caption (docs/command-bar.md), as drawn from the core's answer.
// What no headless test sees: the order the items stand in on screen, New Mail's slot following the
// folder pane, the items the bar no longer carries, and the overflow, which only a measured row
// decides. The bar is one row: New Mail is a button and the actions are a CommandBar's, so they
// share a centre rather than a height.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class CommandBarTests(ShowcaseApp showcase)
{
    // Rule 3's order, by slot.
    private static readonly string[] Actions = ["CommandDelete", "CommandArchive", "CommandMove", "CommandFlag", "CommandRead", "CommandSync"];

    // Physical pixels, slack for rounding rather than design.
    private const double Tolerance = 2;

    private static Element Find(App app, string id) =>
        app.Agent.Wait(new Filter { Id = id, Type = "Button", Window = app.Mailbox.Hwnd, OnScreen = true }, App.Patience) is { Rendered: true } button
            ? app.Steady(button)
            : throw new InvalidOperationException($"the command bar does not draw #{id}");

    private static Dictionary<string, Element> Bar(App app) =>
        Actions.Prepend("ComposeButton").ToDictionary(id => id, id => Find(app, id));

    /// <summary>Whether a message row is selected, read off the list rather than off another button
    /// bound to the same answer: two controls sharing one broken binding agree perfectly.</summary>
    private static bool RowSelected(App app) => app.Rows().Any(row => app.Agent.Read(row).Selected == true);

    private static double Middle(Element e) => e.Y + (e.Height / 2);

    private static void TogglePane(App app)
    {
        app.Agent.Invoke(app.Agent.Find(Filter.ById("PART_PaneToggleButton", "Button")) ?? throw new InvalidOperationException("no pane toggle in the caption"));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
    }

    [OfflineFact]
    public void NewMailThenTheActionsInOutlooksOrderOnOneLine() => showcase.Case(nameof(NewMailThenTheActionsInOutlooksOrderOnOneLine), app =>
    {
        var bar = Bar(app);
        var order = bar.OrderBy(pair => pair.Value.X).Select(pair => pair.Key).ToList();
        Assert.Equal(Actions.Prepend("ComposeButton"), order);
        var centres = bar.Values.Select(Middle).ToList();
        Assert.True(centres.Max() - centres.Min() <= Tolerance,
            $"the bar is one row: {string.Join(" | ", bar.Select(b => $"{b.Key} {b.Value.Y:0}..{b.Value.Y + b.Value.Height:0}"))}");
    });

    /// <summary>Rule 2: the actions stand over the panes they act on, from the message list's leading
    /// edge.</summary>
    [OfflineFact]
    public void TheActionsStartAtTheMessageListsLeadingEdge() => showcase.Case(nameof(TheActionsStartAtTheMessageListsLeadingEdge), app =>
    {
        var delete = Find(app, "CommandDelete");
        var list = app.RowsList();
        var slack = 24 * Input.Scale(app.Mailbox.Hwnd);
        Assert.True(Math.Abs(delete.X - list.X) <= slack,
            $"Delete starts at {delete.X:0}, the message list at {list.X:0}: New Mail's slot is not the folder pane's width");
    });

    /// <summary>Rule 2: New Mail lives on the bar because a pane that can be shut cannot hold it, and
    /// the actions follow the pane rather than staying where it was.</summary>
    [OfflineFact]
    public void ACollapsedFolderPaneKeepsNewMailAndTheActionsFollowIt() => showcase.Case(nameof(ACollapsedFolderPaneKeepsNewMailAndTheActionsFollowIt), app =>
    {
        var open = Find(app, "CommandDelete").X;
        TogglePane(app);
        var compose = Find(app, "ComposeButton");
        var delete = Find(app, "CommandDelete");
        Assert.True(delete.X < open, $"Delete stayed at {delete.X:0} with the folder pane collapsed; it stood at {open:0} beside the open pane");
        Assert.True(delete.X >= compose.X + compose.Width, "and it still stands after New Mail");
    }, TogglePane);

    [OfflineFact]
    public void NewMailAndSyncNameNoSelectionAndTheActionsFollowIt() => showcase.Case(nameof(NewMailAndSyncNameNoSelectionAndTheActionsFollowIt), app =>
    {
        var bar = Bar(app);
        Assert.True(bar["ComposeButton"].Enabled, "New Mail stays live whatever is selected: it writes a message");
        Assert.True(bar["CommandSync"].Enabled, "Sync stays live whatever is selected: it syncs the mailbox");
        Assert.Equal(RowSelected(app), bar["CommandArchive"].Enabled);
    });

    /// <summary>Rule 3: Select all and Clear selection are the list's keys now, and a delete for good
    /// stands only in Delete's place.</summary>
    [OfflineFact]
    public void TheBarCarriesNoSelectionButtonsAndNoSecondDelete() => showcase.Case(nameof(TheBarCarriesNoSelectionButtonsAndNoSecondDelete), app =>
    {
        foreach (var gone in new[] { "SelectionSelectAll", "SelectionClear", "SelectionDeletePermanently" })
        {
            Assert.Null(app.Agent.Find(Filter.ById(gone)));
        }
        // On the bar: the reading pane's own row carries a Delete of its own.
        var bar = app.Agent.Find(Filter.ById("MailCommandBar")) ?? throw new InvalidOperationException("no #MailCommandBar");
        var deletes = app.Agent.FindAll(new Filter { Type = "Button", Under = bar, OnScreen = true })
            .Count(button => button.Name == Catalog.English("action_delete") || button.Name == Catalog.English("action_delete_permanently"));
        Assert.Equal(1, deletes);
    });

    /// <summary>Answers the purge question with Cancel, having checked it is the purge question.</summary>
    private static void CancelPurge(App app, string from)
    {
        var dialog = app.Dialog(Filter.ById("CloseButton", "Button"));
        Assert.True(app.Agent.FindAll(new Filter { Under = dialog }).Any(e => e.Name == Catalog.English("delete_permanently_title")),
            $"{from}'s Delete permanently opened something other than the question before a delete for good");
        app.Agent.Invoke(app.Agent.Find(new Filter { Id = "CloseButton", Type = "Button", Under = dialog })!);
        app.CloseDialogs();
    }

    /// <summary>Rules 6 and 8: a delete for good is asked first, from the bar and from the reading
    /// pane, whose Delete is the bar's over the open message. The showcase's accounts have no Trash,
    /// so Delete is Delete permanently there, which is what this case needs.</summary>
    [OfflineFact]
    public void ADeleteForGoodIsAskedFirstFromTheBarAndTheReadingPane() => showcase.Case(nameof(ADeleteForGoodIsAskedFirstFromTheBarAndTheReadingPane), app =>
    {
        var delete = Find(app, "CommandDelete");
        if (delete.Name != Catalog.English("action_delete_permanently") || !delete.Enabled)
        {
            throw new InvalidOperationException($"the bar's Delete reads '{delete.Name}' (enabled: {delete.Enabled}); this case needs a selection on an account with no Trash, which the showcase seeds");
        }
        var rows = app.Rows().Count;
        app.Agent.Invoke(delete);
        CancelPurge(app, "the bar");
        app.Agent.Invoke(Find(app, "ReadingDelete"));
        CancelPurge(app, "the reading pane");
        Assert.Equal(rows, app.Rows().Count);
    });

    /// <summary>Rule 4: a row too narrow for every item sends them from the end into "More actions",
    /// in bar order, ahead of what always stands there; what stays on the bar does not move.</summary>
    [OfflineFact]
    public void ANarrowWindowSendsTheTrailingItemsIntoMoreInBarOrder() => showcase.Case(nameof(ANarrowWindowSendsTheTrailingItemsIntoMoreInBarOrder), app =>
    {
        var wide = Actions.ToDictionary(id => id, id => Find(app, id).X);
        using (Desktop.Resized(app.Mailbox.Hwnd, 760, 640))
        {
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(400), floor: TimeSpan.FromMilliseconds(600));
            var onBar = Actions.Where(id => app.Agent.Find(new Filter { Id = id, Type = "Button", OnScreen = true }) is { Rendered: true }).ToList();
            Assert.True(onBar.Count < Actions.Length, "at 760 wide every item still fits, so this case measured nothing");
            Assert.Equal(Actions.Take(onBar.Count), onBar);
            var more = app.Agent.Find(new Filter { Id = "MoreButton", Type = "Button", Window = app.Mailbox.Hwnd })
                ?? throw new InvalidOperationException("the bar overflowed and drew no More button");
            Assert.Equal(Catalog.English("a11y_more_actions"), more.Name);
            foreach (var id in onBar.Where(id => id != "CommandSync"))
            {
                Assert.True(Math.Abs(Find(app, id).X - wide[id]) <= Tolerance, $"#{id} moved when the row narrowed");
            }

            app.Agent.Invoke(more);
            var left = Actions.Skip(onBar.Count).Append("CommandReport").ToList();
            var drawn = left.Select(id => app.Agent.Wait(new Filter { Id = id, OnScreen = true }, TimeSpan.FromSeconds(5))
                ?? throw new InvalidOperationException($"#{id} left the bar and is not in More either")).ToList();
            Assert.Equal(left, drawn.OrderBy(e => e.Y).Select(e => e.Id));
            Input.BringToFront(app.Mailbox.Hwnd);
            Input.Press(Input.Escape);
        }
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(400));
    });
}
