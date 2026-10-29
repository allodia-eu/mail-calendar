// Mail dragged onto a folder in the pane moves there (docs/folder-pane.md, rule 24).
//
// WHY HERE. Which rows take a drop is FolderActions', pinned in Mailcal.Tests, but whether a drag
// over a drawn row reaches that code is WinUI's routing: a drag over a NavigationViewItem never
// reaches the NavigationView's own DragOver, so handlers placed there leave every row refusing.
//
// WHY THE HARNESS. The showcase engine performs no mail writes. The harness moves the message on a
// real server and the list shows it. And a real drag, because no automation pattern starts one.
//
// The seed's "Filed under Projects" travels to the Inbox and back, in the order written, so the
// suite leaves the mailbox as it found it.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ImapHarnessCollection.Name)]
[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
[Trait("Dataset", "harness")]
public sealed class FolderDropTests(ImapHarnessApp harness)
{
    private const string Subject = "Filed under Projects";

    private static readonly string Inbox = Catalog.English("folder_inbox");

    private void Case(string name, Action<App, FolderFlow> body) => harness.Case(name, app => body(app, new FolderFlow(app)));

    /// <summary>Drags the fixture's row onto <paramref name="folder"/>'s own line: onto the label,
    /// clear of the chevron and the count at either end, and 20 epx down, because an open folder's
    /// rectangle takes in its subfolders' rows, and the middle of those is one of them.</summary>
    private static void DragOnto(App app, FolderFlow folders, string folder)
    {
        var row = app.RowTitled($"*{Subject}*");
        var target = app.Steady(folders.Row(folder));
        Input.Drag(row,
            (int)(target.X + Math.Min(120, target.Width / 3)),
            (int)(target.Y + Math.Min(target.Height / 2, 20 * Input.Scale(target.Window))));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
    }

    [HarnessFact]
    public void AMessageDroppedOnTheInboxLeavesProjectsAndArrivesInTheInbox() => Case(nameof(AMessageDroppedOnTheInboxLeavesProjectsAndArrivesInTheInbox), (app, folders) =>
    {
        folders.OpenAndSync("Projects");
        if (!folders.WaitRow(Subject))
        {
            throw new InvalidOperationException($"Projects does not show '{Subject}'; was the harness reseeded?");
        }
        DragOnto(app, folders, Inbox);
        Assert.True(folders.WaitRow(Subject, absent: true), "the dropped message left the folder it was dragged from");
        folders.OpenAndSync(Inbox);
        Assert.True(folders.WaitRow(Subject), "and the Inbox it was dropped on shows it");
    });

    [HarnessFact]
    public void DroppedBackOnProjectsItReturnsThere() => Case(nameof(DroppedBackOnProjectsItReturnsThere), (app, folders) =>
    {
        folders.OpenAndSync(Inbox);
        if (!folders.WaitRow(Subject))
        {
            throw new InvalidOperationException($"the Inbox does not show '{Subject}'; the case before this one did not land");
        }
        DragOnto(app, folders, "Projects");
        Assert.True(folders.WaitRow(Subject, absent: true), "the message left the Inbox");
        folders.OpenAndSync("Projects");
        Assert.True(folders.WaitRow(Subject), "and Projects holds it again");
    });
}
