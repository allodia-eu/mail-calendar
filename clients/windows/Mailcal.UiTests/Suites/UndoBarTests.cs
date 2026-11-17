// The bar that offers Undo after a swipe floats over the bottom of the message list, so it is read
// against whatever rows lie under it. InfoBar's informational ground is a near-transparent tint,
// meant for a bar laid out in a row of its own; over the list, the rows' subjects and dates showed
// through it and ran into "Archived" and Undo.
//
// A brush is not an automation property, so this reads the screen: the part of the bar past its Undo
// button holds nothing of the bar's own, so on an opaque bar it is one colour. The same strip with
// the bar gone must show ink of the list's, or the case measured empty space and proves nothing.
//
// The bar is raised through the row menu's Archive, the mouse's path to the swipe (a SwipeControl
// answers to no synthetic pointer), and Undo is pressed before its window lapses, so nothing is
// archived. A flat row, because a conversation's Archive moves at once and raises no bar.

using System.Drawing;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class UndoBarTests(ShowcaseApp showcase)
{
    private const string FlatRow = "Your order has shipped";

    // An opaque ground varies by a few levels at most; a subject's glyph against the list differs
    // from it by over a hundred.
    private const double MaxSpreadOnTheBar = 16;
    private const double MinSpreadOnTheList = 40;

    private static Filter MenuItems(App app) => new() { Type = "MenuItem", Window = app.Mailbox.Hwnd };

    private static void CloseMenu(App app)
    {
        if (app.Agent.Find(MenuItems(app)) is not null)
        {
            Input.Press(Input.Escape);
            app.Agent.WaitGone(MenuItems(app), TimeSpan.FromSeconds(5));
        }
    }

    private static double Spread(long window, Rectangle area)
    {
        var (_, darkest, lightest) = App.Luminance(window, area);
        return lightest - darkest;
    }

    [OfflineFact]
    public void TheListDoesNotShowThroughTheUndoBar() => showcase.Case(nameof(TheListDoesNotShowThroughTheUndoBar), app =>
    {
        // Short enough that the seed's rows run on under the list's bottom edge, where the bar sits.
        using var shrunk = Desktop.Resized(app.Mailbox.Hwnd, 1200, 640);
        app.Agent.Settle();
        var row = app.Steady(app.RowTitled(FlatRow));
        app.ShowMailbox();
        Input.Click(row, right: true);
        var archive = app.Agent.Wait(MenuItems(app) with { Name = Catalog.English("action_archive") }, App.Patience)
            ?? throw new InvalidOperationException("the row's menu offers no Archive");
        app.Agent.Invoke(archive);

        var bar = app.Agent.Wait(Filter.ById("UndoBar") with { Window = app.Mailbox.Hwnd, OnScreen = true }, TimeSpan.FromSeconds(3))
            ?? throw new InvalidOperationException($"archiving '{FlatRow}' from its menu raised no undo bar; is it still a flat row in the showcase seed?");
        var undo = app.Agent.Find(new Filter { Type = "Button", Under = bar, Name = Catalog.English("action_undo") })
            ?? throw new InvalidOperationException("the undo bar carries no Undo button");
        var strip = Rectangle.FromLTRB(
            (int)(undo.X + undo.Width + 8), (int)(bar.Y + (bar.Height * 0.2)),
            (int)(bar.X + bar.Width - 8), (int)(bar.Y + (bar.Height * 0.8)));
        Assert.True(strip.Width >= 40 && strip.Height >= 8, $"the bar leaves no room past Undo to read ({strip}), so this would measure nothing");
        // The bar is in the tree before a slow runner has painted it, so each reading is taken until
        // it settles. A see-through bar never reads as one colour however long it is given, and two
        // seconds leave Undo inside its four-second window.
        var onTheBar = double.MaxValue;
        App.Eventually(() => (onTheBar = Spread(bar.Window, strip)) <= MaxSpreadOnTheBar, TimeSpan.FromSeconds(2));

        app.Agent.Invoke(undo);
        app.Agent.WaitGone(Filter.ById("UndoBar") with { Window = app.Mailbox.Hwnd, OnScreen = true }, TimeSpan.FromSeconds(5));
        Assert.True(App.Eventually(() => app.Agent.Find(new Filter { Type = "ListItem", Under = app.RowsList(), Has = new Filter { TextLike = FlatRow } }) is not null, TimeSpan.FromSeconds(5)),
            $"Undo did not bring '{FlatRow}' back");
        app.Agent.Settle();
        var onTheList = 0.0;
        App.Eventually(() => (onTheList = Spread(bar.Window, strip)) >= MinSpreadOnTheList, TimeSpan.FromSeconds(5));

        Assert.True(onTheList >= MinSpreadOnTheList,
            $"with the bar gone the list draws nothing where it stood (spread {onTheList:F0}), so a see-through bar would pass; the showcase needs rows under the list's bottom edge");
        Assert.True(onTheBar <= MaxSpreadOnTheBar,
            $"the list shows through the undo bar: past its Undo button the bar spans {onTheBar:F0} levels of luminance, where an opaque ground spans a few");
    }, CloseMenu);
}
