// The folder moves the suites that file mail share: a folder's row in alice's tree, opening one and
// asking the server for it, and waiting for a message to arrive in, or leave, the list on screen.

using Allodia.Mailcal.UiTests.Driver;

namespace Allodia.Mailcal.UiTests.Suites;

internal sealed class FolderFlow(App app)
{
    private Agent Agent => app.Agent;

    /// <summary>Alice's row for <paramref name="folder"/>, as it is drawn now: a move re-projects the
    /// pane, so a row read before it may be stale.</summary>
    internal Element Row(string folder)
    {
        var account = Agent.Find(Filter.Named(Harness.Alice, "ListItem") with { Window = app.Mailbox.Hwnd })
            ?? throw new InvalidOperationException($"the pane has no row for {Harness.Alice}");
        return Agent.Find(Filter.Named(folder, "ListItem") with { Under = account })
            ?? throw new InvalidOperationException($"{Harness.Alice} shows no {folder} row; is its tree open?");
    }

    /// <summary>Opens <paramref name="folder"/> and asks the server for it: a move re-reads the
    /// folder it left, not the one it reached, so the destination is current only after a sync,
    /// which is what the user presses too.</summary>
    internal void OpenAndSync(string folder)
    {
        app.OpenFolder(folder);
        Agent.Invoke(Agent.Find(Filter.ById("SelectionSync", "Button")) ?? throw new InvalidOperationException("the toolbar has no Sync button"));
    }

    /// <summary>Waits until the open list does, or with <paramref name="absent"/> does not, show
    /// <paramref name="subject"/>; whether it got there.</summary>
    internal bool WaitRow(string subject, bool absent = false)
    {
        var row = new Filter { Type = "ListItem", Under = app.RowsList(), Has = new Filter { NameLike = $"*{subject}*" } };
        return absent ? Agent.WaitGone(row, TimeSpan.FromSeconds(30)) : Agent.Wait(row, TimeSpan.FromSeconds(30)) is not null;
    }
}
