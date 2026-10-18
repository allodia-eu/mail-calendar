// Move to folder… on a message's row menu (docs/folder-pane.md rule 24, docs/list-selection.md
// rule 12).
//
// WHY HERE. Which folders the picker lists is FolderActions', pinned in Mailcal.Tests, but the
// item's visibility is set by the flyout's Opening handler and the move by its Click, and neither is
// linked into that assembly: an unwired handler leaves the item missing, or present and inert, with
// every unit test green. The harness moves the message on a real server and the list shows it.
//
// The seed's "Filed under Projects" travels to the Inbox and back, in the order written, so the
// suite leaves the mailbox as it found it.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ImapHarnessCollection.Name)]
[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
public sealed class MoveToFolderTests(ImapHarnessApp harness)
{
    private const string Subject = "Filed under Projects";

    private static readonly string Inbox = Catalog.English("folder_inbox");

    private void Case(string name, Action<App> body) => harness.Case(name, body, app => app.CloseDialogs());

    /// <summary>Opens <paramref name="folder"/> and asks the server for it: a move re-reads the
    /// folder it left, not the one it reached, so the destination is current only after a sync.</summary>
    private static void OpenAndSync(App app, string folder)
    {
        app.OpenFolder(folder);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("SelectionSync", "Button")) ?? throw new InvalidOperationException("the toolbar has no Sync button"));
    }

    /// <summary>Waits until the open list does, or with <paramref name="absent"/> does not, show the
    /// fixture; whether it got there.</summary>
    private static bool WaitSubjectRow(App app, bool absent = false)
    {
        var row = new Filter { Type = "ListItem", Under = app.RowsList(), Has = new Filter { NameLike = $"*{Subject}*" } };
        return absent ? app.Agent.WaitGone(row, TimeSpan.FromSeconds(30)) : app.Agent.Wait(row, TimeSpan.FromSeconds(30)) is not null;
    }

    /// <summary>Presses Move to folder… on the fixture's row menu and returns the picker, left open.</summary>
    private static Element OpenPicker(App app)
    {
        var item = app.RowMenuItem(Subject, "action_move_to_folder");
        if (item is null)
        {
            Input.Press(Input.Escape);
            throw new InvalidOperationException($"the row menu offers no '{Catalog.English("action_move_to_folder")}'");
        }
        app.Agent.Invoke(item);
        return app.Agent.Wait(Filter.ById("FolderMoveTargets"), TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("Move to folder… opened no picker");
    }

    private static void MoveTo(App app, string label)
    {
        var list = OpenPicker(app);
        app.Agent.Invoke(app.Agent.Find(new Filter { Type = "ListItem", Name = label, Under = list })
            ?? throw new InvalidOperationException($"the picker does not list '{label}'"));
        app.Agent.WaitGone(Filter.ById("FolderMoveTargets"), TimeSpan.FromSeconds(10));
    }

    private static Element FolderRow(App app, string folder)
    {
        var account = app.Agent.Find(Filter.Named(Harness.Alice, "ListItem") with { Window = app.Mailbox.Hwnd })
            ?? throw new InvalidOperationException($"the pane has no row for {Harness.Alice}");
        return app.Agent.Find(Filter.Named(folder, "ListItem") with { Under = account })
            ?? throw new InvalidOperationException($"{Harness.Alice} shows no {folder} row; is its tree open?");
    }

    [HarnessFact]
    public void ThePickerDrawsTheTreeOfferingTheFoldersThatTakeMailAndNotTheOneOnScreen() =>
        Case(nameof(ThePickerDrawsTheTreeOfferingTheFoldersThatTakeMailAndNotTheOneOnScreen), app =>
        {
            OpenAndSync(app, "Projects");
            if (!WaitSubjectRow(app))
            {
                throw new InvalidOperationException($"Projects does not show '{Subject}'; was the harness reseeded?");
            }
            var list = OpenPicker(app);
            var items = app.Agent.FindAll(new Filter { Type = "ListItem", Under = list });
            // The name, not the glyph beside it, which can surface as text too.
            var byLabel = items.ToDictionary(item => item.Name, item => (Item: item, Text: app.Agent.FindAll(new Filter { Type = "Text", Under = item })[^1]));
            app.CloseDialogs();
            var labels = string.Join(", ", byLabel.Keys);
            Assert.True(byLabel.ContainsKey(Inbox), $"the Inbox is offered, got: {labels}");
            // The folder on screen is no destination, but holds one, so it stays as its parent (rule 24).
            Assert.True(byLabel.ContainsKey("Projects"), $"Projects is drawn above what is inside it, got: {labels}");
            Assert.False(byLabel["Projects"].Item.Enabled, "but it is not offered");
            Assert.True(byLabel["Projects / Clients"].Item.Enabled, "its subfolder is");
            // Drawn as the pane draws it: the folder's own name, indented under its parent.
            Assert.True(byLabel["Projects / Clients"].Text.Name == "Clients", "a row shows the folder's own name, not its path");
            Assert.True(byLabel["Projects / Clients"].Text.X > byLabel["Projects"].Text.X, "a subfolder sits one step in");
            Assert.True(byLabel["Projects / Clients / Acme"].Text.X > byLabel["Projects / Clients"].Text.X, "and its own subfolder another");
            // Both are in the pane, so leaving them out of the picker is the rule and not the seed.
            foreach (var key in new[] { "folder_junk", "folder_drafts" })
            {
                var folder = Catalog.English(key);
                FolderRow(app, folder);
                Assert.False(byLabel.ContainsKey(folder), $"{folder} takes no move (rule 24)");
            }
        });

    [HarnessFact]
    public void MovedToTheInboxItLeavesProjectsAndArrivesThere() => Case(nameof(MovedToTheInboxItLeavesProjectsAndArrivesThere), app =>
    {
        OpenAndSync(app, "Projects");
        if (!WaitSubjectRow(app))
        {
            throw new InvalidOperationException($"Projects does not show '{Subject}'");
        }
        MoveTo(app, Inbox);
        Assert.True(WaitSubjectRow(app, absent: true), "the message left the folder it was moved from");
        OpenAndSync(app, Inbox);
        Assert.True(WaitSubjectRow(app), "and the Inbox shows it");
    });

    [HarnessFact]
    public void MovedBackToProjectsItReturnsThere() => Case(nameof(MovedBackToProjectsItReturnsThere), app =>
    {
        OpenAndSync(app, Inbox);
        if (!WaitSubjectRow(app))
        {
            throw new InvalidOperationException($"the Inbox does not show '{Subject}'; the case before this one did not land");
        }
        MoveTo(app, "Projects");
        Assert.True(WaitSubjectRow(app, absent: true), "the message left the Inbox");
        OpenAndSync(app, "Projects");
        Assert.True(WaitSubjectRow(app), "and Projects holds it again");
    });
}
