// The folder pane's row menus as the user gets them (docs/folder-pane.md, rules 22 to 24).
//
// WHY HERE. Mailcal.Tests pins which items a row's flags produce (FolderActionsTests), but not that
// the menu reaches a drawn row at all. The menu is built in code on the NavigationView's
// ContextRequested, because a ContextFlyout declared in the item template never opens
// (MainWindow.Sidebar.cs), and that route is the kind that breaks while every unit test stays green.
//
// WHY THE HARNESS. The showcase engine cannot change folders, so its pane correctly offers nothing.
//
// Nothing here changes a folder: the menus and the Move to… picker are opened, read and shut. Making,
// renaming, moving and deleting are the core's, proven in tests_folder_ops.rs (mailcal-app).

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class FolderMenuTests(HarnessApp harness)
{
    private static readonly string TopLevel = Catalog.English("folder_move_top_level");

    private void Case(string name, Action<App> body) => harness.Case(name, body, CloseMenu);

    private static Filter MenuItems(App app) => new() { Type = "MenuItem", Window = app.Mailbox.Hwnd };

    private static void CloseMenu(App app)
    {
        if (app.Agent.Find(MenuItems(app)) is not null)
        {
            Input.BringToFront(app.Mailbox.Hwnd);
            Input.Press(Input.Escape);
            app.Agent.WaitGone(MenuItems(app), TimeSpan.FromSeconds(5));
        }
    }

    /// <summary>
    /// Right-clicks <paramref name="row"/> and returns the items its menu offers, left open, or none
    /// once a moment has shown it raised no menu. On the row's own header line, never its middle:
    /// an expanded row's rectangle takes in its whole subtree, whose middle is one of its folders.
    /// </summary>
    private static IReadOnlyList<Element> OpenMenu(App app, Element row)
    {
        row = app.Steady(row);
        var header = app.Agent.FindAll(new Filter { Type = "ListItem", Under = row })
            .Where(child => child.Rendered && child.Y > row.Y)
            .Select(child => child.Y - row.Y)
            .DefaultIfEmpty(row.Height)
            .Min();
        Input.BringToFront(app.Mailbox.Hwnd);
        Input.ClickAt((int)(row.X + 20 * Input.Scale(row.Window)), (int)(row.Y + header / 2), right: true);
        if (app.Agent.Wait(MenuItems(app), TimeSpan.FromSeconds(2)) is null)
        {
            return [];
        }
        app.Agent.Settle();
        return app.Agent.FindAll(MenuItems(app));
    }

    private static List<string> MenuOf(App app, Element row)
    {
        var names = OpenMenu(app, row).Select(item => item.Name).ToList();
        CloseMenu(app);
        return names;
    }

    private static Element AccountRow(App app) =>
        app.Agent.Wait(Filter.Named(Harness.Alice, "ListItem") with { Window = app.Mailbox.Hwnd }, App.Patience)
        ?? throw new InvalidOperationException($"the pane has no row for {Harness.Alice}");

    /// <summary>Alice's row for <paramref name="path"/>, each folder on the way opened so the row is
    /// drawn. A launch lists the account before its folders arrive from the server.</summary>
    private static Element FolderRow(App app, params string[] path)
    {
        var row = AccountRow(app);
        foreach (var name in path)
        {
            app.Agent.Expand(row);
            row = app.Agent.Wait(Filter.Named(name, "ListItem") with { Under = row, OnScreen = true }, App.Patience)
                ?? throw new InvalidOperationException($"{Harness.Alice} shows no {string.Join(" / ", path)} row");
        }
        return row;
    }

    /// <summary>
    /// Presses Move to… on <paramref name="row"/> and returns the picker's rows, label to whether
    /// it is offered, in order, then cancels it. Throws unless the picker is <paramref name="name"/>'s,
    /// so a click that landed on a neighbouring row cannot pass as this folder's answer.
    /// </summary>
    private static List<(string Label, bool Enabled)> MoveTargets(App app, Element row, string name)
    {
        var move = Catalog.English("folder_action_move");
        var item = OpenMenu(app, row).FirstOrDefault(i => i.Name == move);
        if (item is null)
        {
            CloseMenu(app);
            throw new InvalidOperationException($"the row menu offers no '{move}'");
        }
        app.Agent.Invoke(item);
        var list = app.Agent.Wait(Filter.ById("FolderMoveTargets"), TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("Move to… opened no picker");
        var targets = app.Agent.FindAll(new Filter { Type = "ListItem", Under = list }).Select(t => (t.Name, t.Enabled)).ToList();
        app.CloseDialogs();
        var expected = Catalog.English("folder_move_title").Replace("{name}", name);
        return list.Name == expected ? targets : throw new InvalidOperationException($"the picker opened for the wrong folder: '{list.Name}', not '{expected}'");
    }

    [HarnessFact]
    public void AnAccountWhoseFoldersCanChangeOffersNewFolderBesideRemoveAccount() => Case(nameof(AnAccountWhoseFoldersCanChangeOffersNewFolderBesideRemoveAccount), app =>
    {
        var menu = MenuOf(app, AccountRow(app));
        Assert.True(menu.Count == 2, $"the account row offers exactly New folder and Remove account, got: {string.Join(", ", menu)}");
        Assert.Equal(Catalog.English("folder_action_new"), menu[0]);
        Assert.Equal(Catalog.English("action_remove_account"), menu[1]);
    });

    [HarnessFact]
    public void TheInboxTakesANewFolderAndOffersNoRenameMoveOrDelete() => Case(nameof(TheInboxTakesANewFolderAndOffersNoRenameMoveOrDelete), app =>
    {
        // The account's own Inbox, not the unified one under All Accounts, which belongs to no account.
        var menu = MenuOf(app, FolderRow(app, Catalog.English("folder_inbox")));
        Assert.True(menu.Count == 1, $"a role folder is named and placed by the app, got: {string.Join(", ", menu)}");
        Assert.Equal(Catalog.English("folder_action_new"), menu[0]);
    });

    [HarnessFact]
    public void TheUnifiedInboxRaisesNoMenu() => Case(nameof(TheUnifiedInboxRaisesNoMenu), app =>
    {
        var group = app.Agent.Find(Filter.Named(Catalog.English("sidebar_all_accounts"), "ListItem") with { Window = app.Mailbox.Hwnd })
            ?? throw new InvalidOperationException("the pane has no All Accounts group");
        var unified = app.Agent.Find(Filter.Named(Catalog.English("folder_inbox"), "ListItem") with { Under = group })
            ?? throw new InvalidOperationException("the All Accounts group shows no Inbox row");
        Assert.Empty(MenuOf(app, unified));
    });

    [HarnessFact]
    public void MoveToOnAFolderInsideAnotherLeavesOutTheFolderItIsIn() => Case(nameof(MoveToOnAFolderInsideAnotherLeavesOutTheFolderItIsIn), app =>
    {
        var targets = MoveTargets(app, FolderRow(app, "Archive", "2025"), "2025");
        var offered = targets.ToDictionary(t => t.Label, t => t.Enabled);
        var labels = string.Join(", ", targets.Select(t => t.Label));
        Assert.True(targets[0].Label == TopLevel, $"Top level is the root, got: {labels}");
        Assert.True(offered[TopLevel], "and is offered, since the folder is not there");
        Assert.False(offered.ContainsKey("Archive / 2025"), "the folder itself is no destination");
        // Archive holds 2026, a destination, so it stays as 2026's parent, but not as a choice.
        Assert.True(offered.ContainsKey("Archive"), $"Archive is drawn above 2026, got: {labels}");
        Assert.False(offered["Archive"], "but the folder 2025 already sits in is not offered");
        Assert.True(offered["Archive / 2026"], "its sibling is");
        Assert.True(offered["Projects"], "and so is a folder elsewhere");
    });

    [HarnessFact]
    public void MoveToOnATopLevelFolderKeepsTopLevelAsTheRootNotOffered() => Case(nameof(MoveToOnATopLevelFolderKeepsTopLevelAsTheRootNotOffered), app =>
    {
        var targets = MoveTargets(app, FolderRow(app, "Archive"), "Archive");
        var offered = targets.ToDictionary(t => t.Label, t => t.Enabled);
        var labels = string.Join(", ", targets.Select(t => t.Label));
        Assert.True(targets[0].Label == TopLevel, $"Top level is still the root, got: {labels}");
        Assert.False(offered[TopLevel], "but Archive is already there");
        Assert.False(offered.ContainsKey("Archive"), "the folder itself is no destination");
        Assert.False(offered.ContainsKey("Archive / 2026"), "nor is anything inside it");
        Assert.True(offered["Projects"], $"another top-level folder is, got: {labels}");
    });
}
