// What the sidebar reconcile COSTS, as arithmetic.
//
// The NavigationView is bound to the collection SidebarTree reconciles, so every collection event
// reaching it is a container the framework must realise. The sidebar used to be rebuilt by hand
// instead (MenuItems.Clear() + a fresh NavigationViewItem tree) once per event, and the model
// refills its collections with Clear() + one Add() per entry, so opening a real 57-folder account
// meant sixty full rebuilds and ten seconds of frozen UI thread, sixteen on the second visit. The
// core's half of the same refresh takes 2 ms.
//
// None of that is visible to a screenshot or to UI Automation: both see the correct sidebar either
// way, only slower. So it is asserted here, TreeCounter records every event the framework would
// have heard, which is exactly the kind of check that could not fail before, because there was
// nothing counting.
//
// The pane's own rules are SidebarTreeTests, and the fixtures both use are SidebarFixture.

using System.Collections.ObjectModel;
using Allodia.Mailcal.ViewModels;
using Xunit;

using static Allodia.Mailcal.Tests.SidebarFixture;

namespace Allodia.Mailcal.Tests;

public class SidebarTreeCostTests
{
    // Counts every collection event anywhere in the tree, the stand-in for the containers the
    // NavigationView realises, since a real one needs a window.
    //
    // It has to follow the CHILDREN collections, not just the root: an account's folders are the
    // ones that churn, and they live on that account's item. A counter watching only the root
    // reports a confident zero while fifty-seven folder rows are being rebuilt, which is exactly
    // what the first draft of this file did.
    private sealed class TreeCounter
    {
        // ObservableCollection doesn't override Equals, so the default comparer is reference
        // identity, which is what "have I already hooked this one?" means here.
        private readonly HashSet<ObservableCollection<SidebarItem>> _hooked = [];

        public TreeCounter(ObservableCollection<SidebarItem> root) => Hook(root);

        public int Events { get; private set; }

        private void Hook(ObservableCollection<SidebarItem> collection)
        {
            if (!_hooked.Add(collection))
            {
                return;
            }
            collection.CollectionChanged += (_, e) =>
            {
                Events++;
                foreach (var item in e.NewItems?.Cast<SidebarItem>() ?? [])
                {
                    Hook(item.Children);
                }
            };
            foreach (var item in collection)
            {
                Hook(item.Children);
            }
        }
    }

    [Fact]
    public void A_refresh_that_changes_nothing_raises_nothing()
    {
        var target = new ObservableCollection<SidebarItem>();
        var counter = new TreeCounter(target);
        var accounts = new List<AccountItem> { Account("a", folders: Folders(20)), Account("b") };
        Sync(target, accounts, selected: "a");
        var settled = counter.Events;

        // A background sync re-signals the same state many times a second. Each one used to tear
        // the whole menu down and build it again.
        for (var i = 0; i < 10; i++)
        {
            Sync(target, accounts, selected: "a");
        }

        Assert.Equal(settled, counter.Events);
        Assert.Equal(20, target[1].Children.Count);
    }

    [Fact]
    public void A_count_that_moves_costs_a_property_change_not_a_container()
    {
        // New mail arrives constantly, and it moves a number on a row that is otherwise identical.
        // If that went through the collection, every delivery would re-realise the folder tree.
        var target = new ObservableCollection<SidebarItem>();
        var counter = new TreeCounter(target);
        Sync(target, [Account("a", folders: [Folder("inbox", "Inbox", SidebarFolderRole.Inbox, 3)])]);
        var inbox = target[1].Children[0];
        var settled = counter.Events;

        Sync(
            target,
            [Account("a", folders: [Folder("inbox", "Inbox", SidebarFolderRole.Inbox, 4)])],
            unifiedUnread: 4);

        Assert.Equal(4u, inbox.Unread);
        Assert.Same(inbox, target[1].Children[0]);
        Assert.Equal(settled, counter.Events);
    }

    [Fact]
    public void An_unchanged_entry_keeps_its_identity_across_a_refresh()
    {
        var target = new ObservableCollection<SidebarItem>();
        Sync(target, [Account("a", folders: Folders(5)), Account("b")], selected: "a");
        var account = target[1];
        var folder = account.Children[2];

        // A folder renamed elsewhere in the list must not cost the others their containers.
        var renamed = Folders(5);
        renamed[1] = Folder("f0", "Renamed");
        Sync(target, [Account("a", folders: renamed), Account("b")], selected: "a");

        Assert.Same(account, target[1]);
        Assert.Same(folder, target[1].Children[2]);
        Assert.Equal("Renamed", target[1].Children[0].Content);
    }

    [Fact]
    public void The_outage_badge_flips_in_place_rather_than_replacing_the_row()
    {
        var target = new ObservableCollection<SidebarItem>();
        var counter = new TreeCounter(target);
        Sync(target, Accounts("a", "b"));
        var account = target[1];
        var settled = counter.Events;

        Sync(target, Accounts("a", "b"), unreachable: id => id == "a");

        Assert.True(account.ShowBadge);
        Assert.Same(account, target[1]);
        // A re-badge is a property change on a row that stays put, not a collection event.
        Assert.Equal(settled, counter.Events);
    }

    [Fact]
    public void A_new_account_arrives_with_its_folders_already_attached()
    {
        // The cost bound, restated for the shape the pane has now, and it got cheaper. An
        // account's folders are assembled onto its row BEFORE the row joins the bound collection,
        // so a 57-folder account joining is ONE event: the framework realises the row and reads
        // its children itself. The core builds `account_folders` in a single pass, so there is no
        // per-folder burst left to pay for.
        const int FolderCount = 57;
        var target = new ObservableCollection<SidebarItem>();
        var counter = new TreeCounter(target);
        Sync(target, [Account("a", folders: Folders(9))]);
        var before = counter.Events;

        Sync(target, [Account("a", folders: Folders(9)), Account("b", folders: Folders(FolderCount))]);

        Assert.Equal(1, counter.Events - before);
        Assert.Equal(9, target[1].Children.Count);
        Assert.Equal(FolderCount, target[2].Children.Count);
    }

    [Fact]
    public void Growing_an_open_accounts_folder_list_stays_linear_in_what_arrived()
    {
        // The case the counter can still see, and the one the original regression was about: an
        // account already on screen, whose folders a sync then fills in. Rebuilding instead of
        // reconciling made this quadratic, 60 teardowns of a ~60-item tree, ~3,500 controls, ten
        // seconds of frozen UI thread. Reconciling touches only the rows that actually arrived.
        const int FolderCount = 57;
        var target = new ObservableCollection<SidebarItem>();
        var counter = new TreeCounter(target);
        Sync(target, [Account("a", folders: Folders(9))]);
        var settled = counter.Events;

        // The folder list lands one chunk at a time, as a streaming sync commits it.
        var all = Folders(FolderCount);
        for (var i = 9; i <= FolderCount; i++)
        {
            Sync(target, [Account("a", folders: all.Take(i + 1).ToList())]);
        }

        var spent = counter.Events - settled;
        // One insert per folder that actually appeared (57 − 9), and not one event more: the
        // folders already on screen keep their containers through every one of those refreshes.
        Assert.Equal(FolderCount - 9, spent);
        Assert.InRange(spent, 0, 2 * FolderCount);
        Assert.Equal(FolderCount, target[1].Children.Count);
    }

    [Fact]
    public void A_removed_account_takes_its_row_with_it()
    {
        var target = new ObservableCollection<SidebarItem>();
        Sync(target, [Account("a"), Account("b", folders: Folders(2)), Account("c")], selected: "b");
        Sync(target, [Account("a"), Account("c")]);

        Assert.Equal(
            [SidebarTree.AllAccountsTag, "acct:a", "acct:c", SidebarTree.AddAccountTag],
            target.Select(i => i.Tag));
    }

    [Fact]
    public void Reordered_accounts_are_moved_rather_than_rebuilt()
    {
        var target = new ObservableCollection<SidebarItem>();
        Sync(target, Accounts("a", "b", "c"));
        var a = target[1];
        var c = target[3];

        // The core adds accounts as they connect, so the order can genuinely change between boots.
        Sync(target, Accounts("c", "a", "b"));

        Assert.Equal(
            [SidebarTree.AllAccountsTag, "acct:c", "acct:a", "acct:b", SidebarTree.AddAccountTag],
            target.Select(i => i.Tag));
        Assert.Same(c, target[1]);
        Assert.Same(a, target[2]);
    }}
