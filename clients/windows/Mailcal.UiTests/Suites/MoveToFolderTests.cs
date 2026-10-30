// Move to folder… on a message's row menu (docs/folder-pane.md rule 24, docs/list-selection.md
// rule 12).
//
// WHY HERE. Which folders the picker lists is FolderActions', pinned in Mailcal.Tests, but the
// item is drawn by the flyout's Opening handler and the move run by its Click, and neither is
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
[Trait("Dataset", "harness")]
public sealed class MoveToFolderTests(ImapHarnessApp harness)
{
    private const string Subject = "Filed under Projects";

    private static readonly string Inbox = Catalog.English("folder_inbox");

    private void Case(string name, Action<App, FolderFlow> body) =>
        harness.Case(name, app => body(app, new FolderFlow(app)), app => app.CloseDialogs());

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

    [HarnessFact]
    public void ThePickerDrawsTheTreeOfferingTheFoldersThatTakeMailAndNotTheOneOnScreen() =>
        Case(nameof(ThePickerDrawsTheTreeOfferingTheFoldersThatTakeMailAndNotTheOneOnScreen), (app, folders) =>
        {
            folders.OpenAndSync("Projects");
            if (!folders.WaitRow(Subject))
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
                folders.Row(folder);
                Assert.False(byLabel.ContainsKey(folder), $"{folder} takes no move (rule 24)");
            }
        });

    [HarnessFact]
    public void MovedToTheInboxItLeavesProjectsAndArrivesThere() => Case(nameof(MovedToTheInboxItLeavesProjectsAndArrivesThere), (app, folders) =>
    {
        folders.OpenAndSync("Projects");
        if (!folders.WaitRow(Subject))
        {
            throw new InvalidOperationException($"Projects does not show '{Subject}'");
        }
        MoveTo(app, Inbox);
        Assert.True(folders.WaitRow(Subject, absent: true), "the message left the folder it was moved from");
        folders.OpenAndSync(Inbox);
        Assert.True(folders.WaitRow(Subject), "and the Inbox shows it");
    });

    [HarnessFact]
    public void MovedBackToProjectsItReturnsThere() => Case(nameof(MovedBackToProjectsItReturnsThere), (app, folders) =>
    {
        folders.OpenAndSync(Inbox);
        if (!folders.WaitRow(Subject))
        {
            throw new InvalidOperationException($"the Inbox does not show '{Subject}'; the case before this one did not land");
        }
        MoveTo(app, "Projects");
        Assert.True(folders.WaitRow(Subject, absent: true), "the message left the Inbox");
        folders.OpenAndSync("Projects");
        Assert.True(folders.WaitRow(Subject), "and Projects holds it again");
    });
}
