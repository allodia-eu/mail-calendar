// How the model's account list moves onto a new snapshot, observed through the pane.
//
// MainWindow reconciles the folder pane on every event Model.Accounts raises, so what the pane
// draws between two snapshots is every intermediate state of that collection. Refilled with
// Clear() + Add(), the pane saw an empty account list after every unread count that moved, took
// every account row and folder tree down and built them again collapsed, and the pane scrolled
// under the reader a moment after each message they dragged onto a folder.
//
// The pane's own rules are SidebarTreeTests, its cost SidebarTreeCostTests.

using System.Collections.ObjectModel;
using Allodia.Mailcal.ViewModels;
using Xunit;

using static Allodia.Mailcal.Tests.SidebarFixture;

namespace Allodia.Mailcal.Tests;

public class AccountListTests
{
    // The model's collection wired to the pane as MainWindow wires it, recording the pane's
    // account rows after every event.
    private sealed class WiredPane
    {
        public ObservableCollection<AccountItem> Accounts { get; } = [];

        public ObservableCollection<SidebarItem> Pane { get; } = [];

        public List<string[]> Seen { get; } = [];

        public WiredPane() =>
            Accounts.CollectionChanged += (_, _) =>
            {
                Sync(Pane, Accounts);
                Seen.Add([.. Pane.Where(i => i.AccountId is not null).Select(i => i.AccountId!)]);
            };

        public void Show(IReadOnlyList<AccountItem> wanted) =>
            AccountList.Update(Accounts, wanted, SameDrawing);
    }

    // What MailboxModel compares, reduced to what these cases move.
    private static bool SameDrawing(AccountItem a, AccountItem b) =>
        a.Id == b.Id
        && a.Expanded == b.Expanded
        && a.Folders.Count == b.Folders.Count
        && a.Folders.Zip(b.Folders).All(pair => pair.First.Key == pair.Second.Key && pair.First.Unread == pair.Second.Unread);

    private static List<FolderItem> WithUnread(int count, int at, uint unread)
    {
        var folders = Folders(count);
        folders[at + 1] = Folder($"f{at}", $"Folder {at}", unread: unread);
        return folders;
    }

    [Fact]
    public void A_count_that_moves_never_takes_an_account_off_the_pane()
    {
        var wired = new WiredPane();
        wired.Show([Account("a", folders: Folders(20)), Account("b", folders: Folders(3))]);
        var row = wired.Pane[1];
        // The synthetic "All Mail" head draws no row, so f3 is the fourth child.
        var folder = row.Children[3];
        wired.Seen.Clear();

        // A message dropped on f3 moves its count; the snapshot after the move carries it.
        wired.Show([Account("a", folders: WithUnread(20, 3, 1)), Account("b", folders: Folders(3))]);

        Assert.NotEmpty(wired.Seen);
        Assert.All(wired.Seen, pane => Assert.Equal(["a", "b"], pane));
        Assert.Same(row, wired.Pane[1]);
        Assert.Same(folder, wired.Pane[1].Children[3]);
        Assert.Equal(1u, wired.Pane[1].Children[3].Unread);
    }

    [Fact]
    public void An_unchanged_snapshot_raises_nothing()
    {
        var wired = new WiredPane();
        var accounts = new List<AccountItem> { Account("a", folders: Folders(5)), Account("b") };
        wired.Show(accounts);
        wired.Seen.Clear();

        wired.Show([Account("a", folders: Folders(5)), Account("b")]);

        Assert.Empty(wired.Seen);
    }

    [Fact]
    public void Only_the_account_that_changed_is_replaced()
    {
        var wired = new WiredPane();
        wired.Show([Account("a", folders: Folders(2)), Account("b", folders: Folders(2))]);
        var untouched = wired.Accounts[0];

        wired.Show([Account("a", folders: Folders(2)), Account("b", folders: WithUnread(2, 0, 5))]);

        Assert.Same(untouched, wired.Accounts[0]);
        Assert.Equal(5u, wired.Accounts[1].Folders[1].Unread);
    }

    [Fact]
    public void Accounts_arriving_leaving_and_reordering_pass_only_through_real_states()
    {
        var wired = new WiredPane();
        wired.Show(Accounts("a", "b", "c"));
        wired.Seen.Clear();

        wired.Show(Accounts("c", "d", "a"));

        Assert.Equal(["c", "d", "a"], wired.Accounts.Select(a => a.Id));
        Assert.Equal(["c", "d", "a"], wired.Seen[^1]);
        // Never empty, never an account twice, never one that is in neither snapshot.
        Assert.All(wired.Seen, pane =>
        {
            Assert.NotEmpty(pane);
            Assert.Equal(pane.Length, pane.Distinct().Count());
            Assert.All(pane, id => Assert.Contains(id, new[] { "a", "b", "c", "d" }));
        });
    }
}
