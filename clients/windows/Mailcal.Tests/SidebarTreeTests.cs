// The sidebar accordion's shape, and, the regression these exist for, what it COSTS.
//
// The NavigationView is bound to the collection SidebarTree reconciles, so every collection event
// reaching it is a container the framework must realise. The sidebar used to be rebuilt by hand
// instead (MenuItems.Clear() + a fresh NavigationViewItem tree) once per event, and the model
// refills its collections with Clear() + one Add() per entry, so opening a real 57-folder account
// meant sixty full rebuilds and ten seconds of frozen UI thread, sixteen on the second visit. The
// core's half of the same refresh takes 2 ms.
//
// None of that is visible to a screenshot or to UI Automation: both see the correct sidebar either
// way, only slower. So the cost is asserted here as arithmetic, TreeCounter records every
// event the framework would have heard, which is exactly the kind of check that could not fail
// before, because there was nothing counting.
//
// The pane's own rules (docs/folder-pane.md) are pinned here too, and they are the kind that fail
// silently: an expansion driven off the selection looks perfectly reasonable until you notice the
// tree emptying itself, and an unread badge that never updates looks like a folder with no new mail.

using System.Collections.ObjectModel;
using Allodia.Mailcal.ViewModels;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SidebarTreeTests
{
    private static readonly SidebarLabels Labels =
        new("All Inboxes", "Add account…", count => $"{count} unread");

    private static readonly SidebarGlyphs Glyphs = new(
        "all-inboxes",
        "account",
        "folder",
        "add",
        role => role switch
        {
            SidebarFolderRole.Inbox => "inbox-glyph",
            SidebarFolderRole.Sent => "sent-glyph",
            SidebarFolderRole.Trash => "trash-glyph",
            _ => "folder",
        });

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

    private static FolderItem Folder(string key, string name, SidebarFolderRole role = SidebarFolderRole.None, uint unread = 0) =>
        new() { Key = key, Name = name, Role = role, Unread = unread };

    /// <summary>The folder list as the model projects one: the synthetic null-key "All Mail" head,
    /// then the real folders.</summary>
    private static List<FolderItem> Folders(int count) =>
        [
            new FolderItem { Key = null, Name = "All Mail" },
            .. Enumerable.Range(0, count).Select(i => Folder($"f{i}", $"Folder {i}")),
        ];

    private static AccountItem Account(
        string id,
        bool expanded = true,
        IReadOnlyList<FolderItem>? folders = null) =>
        new()
        {
            Id = id,
            Email = id + "@example.com",
            Expanded = expanded,
            Folders = folders ?? [],
        };

    private static List<AccountItem> Accounts(params string[] ids) =>
        [.. ids.Select(id => Account(id))];

    private static void Sync(
        ObservableCollection<SidebarItem> target,
        IReadOnlyList<AccountItem> accounts,
        string? selected = null,
        bool showFolders = true,
        uint unifiedUnread = 0,
        Func<string, bool>? unreachable = null,
        Action<SidebarItem>? onExpanded = null) =>
        SidebarTree.Reconcile(
            target,
            accounts,
            selected,
            showFolders,
            unifiedUnread,
            unreachable ?? (_ => false),
            onExpanded ?? (_ => { }),
            Labels,
            Glyphs);

    [Fact]
    public void Builds_all_inboxes_then_accounts_then_add_account()
    {
        var target = new ObservableCollection<SidebarItem>();
        Sync(target, Accounts("a", "b"));

        Assert.Equal(
            [SidebarTree.AllInboxesTag, "acct:a", "acct:b", SidebarTree.AddAccountTag],
            target.Select(i => i.Tag));
        Assert.Equal("All Inboxes", target[0].Content);
        Assert.Equal("a@example.com", target[1].Content);
        // Add account is an action, not a destination, so it must never hold the selection.
        Assert.False(target[^1].SelectsOnInvoked);
        Assert.True(target[1].SelectsOnInvoked);
    }

    [Fact]
    public void Every_account_carries_its_own_folders_not_just_the_selected_one()
    {
        // The regression this replaced: the pane was fed the SELECTED account's folders alone, so
        // choosing All Inboxes, or the account next to it, emptied the tree on screen.
        var target = new ObservableCollection<SidebarItem>();
        var accounts = new List<AccountItem>
        {
            Account("a", folders: Folders(3)),
            Account("b", folders: Folders(2)),
        };

        Sync(target, accounts, selected: "b");

        // Both trees are on screen, and the synthetic "All Mail" head is not a row of its own.
        Assert.Equal(["f0", "f1", "f2"], target[1].Children.Select(c => c.Tag));
        Assert.Equal(["f0", "f1"], target[2].Children.Select(c => c.Tag));

        // …and with nothing selected at all.
        Sync(target, accounts, selected: null);
        Assert.Equal(3, target[1].Children.Count);
        Assert.Equal(2, target[2].Children.Count);
    }

    [Fact]
    public void Every_folder_names_the_account_it_belongs_to()
    {
        // A folder's tag is its raw provider key, and a key is unique only within its account,
        // both accounts below call their folders f0/f1, exactly as every provider calls its inbox
        // `inbox`. So the shell cannot open a folder from the tag alone: it needs the account, and
        // this is where the row carries it (docs/folder-pane.md, rule 14).
        var target = new ObservableCollection<SidebarItem>();
        var accounts = new List<AccountItem>
        {
            Account("a", folders: Folders(2)),
            Account("b", folders: Folders(2)),
        };

        Sync(target, accounts, selected: null);

        Assert.All(target[1].Children, child => Assert.Equal("a", child.OwnerAccountId));
        Assert.All(target[2].Children, child => Assert.Equal("b", child.OwnerAccountId));
        // Only a folder carries it. An account row names itself through AccountId, which is also
        // the template discriminator, a folder answering to that one would inherit the account
        // row's "Remove account" menu.
        Assert.Null(target[1].OwnerAccountId);
        Assert.Null(target[0].OwnerAccountId);
    }

    [Fact]
    public void Expansion_comes_from_the_core_not_from_the_selection()
    {
        var target = new ObservableCollection<SidebarItem>();
        var accounts = new List<AccountItem>
        {
            Account("a", expanded: true, folders: Folders(2)),
            Account("b", expanded: false, folders: Folders(2)),
        };

        // "b" is selected and still shut; "a" is unselected and still open. Under the old rule
        // those were both impossible, expansion WAS the selection.
        Sync(target, accounts, selected: "b");

        Assert.True(target[1].IsExpanded);
        Assert.False(target[2].IsExpanded);
    }

    [Fact]
    public void Leaving_mail_hides_the_folders_without_forgetting_which_were_open()
    {
        var target = new ObservableCollection<SidebarItem>();
        var accounts = new List<AccountItem> { Account("a", folders: Folders(3)) };
        var toggles = new List<(string Id, bool Expanded)>();

        Sync(target, accounts, showFolders: true, onExpanded: i => toggles.Add((i.AccountId!, i.IsExpanded)));
        Sync(target, accounts, showFolders: false, onExpanded: i => toggles.Add((i.AccountId!, i.IsExpanded)));

        Assert.Empty(target[1].Children);
        Assert.False(target[1].IsExpanded);
        // Crucially, the core was never told the user shut anything, so coming back to mail
        // restores the tree rather than reopening a collapsed one.
        Assert.Empty(toggles);

        Sync(target, accounts, showFolders: true);
        Assert.True(target[1].IsExpanded);
        Assert.Equal(3, target[1].Children.Count);
    }

    [Fact]
    public void A_users_chevron_reaches_the_core_but_a_refresh_does_not()
    {
        var target = new ObservableCollection<SidebarItem>();
        var accounts = new List<AccountItem> { Account("a", folders: Folders(2)) };
        var toggles = new List<(string Id, bool Expanded)>();
        void Refresh() => Sync(target, accounts, onExpanded: i => toggles.Add((i.AccountId!, i.IsExpanded)));

        Refresh();
        Assert.Empty(toggles);

        // The two-way binding writing back what the user clicked.
        target[1].IsExpanded = false;

        Assert.Equal([("a", false)], toggles);

        // A background sync re-publishing the same snapshot must not echo that back as another
        // user action, which would be a dispatch, a rebuild and a reconcile per account per refresh.
        for (var i = 0; i < 5; i++)
        {
            Refresh();
        }
        Assert.Single(toggles);
    }

    [Fact]
    public void Unread_counts_land_on_folders_and_on_all_inboxes_and_vanish_at_zero()
    {
        var target = new ObservableCollection<SidebarItem>();
        var accounts = new List<AccountItem>
        {
            Account("a", folders:
            [
                new FolderItem { Key = null, Name = "All Mail" },
                Folder("inbox", "Inbox", SidebarFolderRole.Inbox, 545),
                Folder("sent", "Sent", SidebarFolderRole.Sent),
                Folder("trash", "Trash", SidebarFolderRole.Trash, 4),
            ]),
        };

        Sync(target, accounts, unifiedUnread: 548);

        var inbox = target[1].Children[0];
        Assert.Equal(545u, inbox.Unread);
        Assert.True(inbox.ShowUnread);
        Assert.Equal("545", inbox.UnreadText);
        Assert.Equal("545 unread", inbox.UnreadLabel);

        // A counted-and-empty folder shows nothing, and carries no label for a screen reader to
        // read out either.
        var sent = target[1].Children[1];
        Assert.False(sent.ShowUnread);
        Assert.Equal(string.Empty, sent.UnreadLabel);

        Assert.Equal(4u, target[1].Children[2].Unread);
        Assert.Equal(548u, target[0].Unread);
        Assert.True(target[0].ShowUnread);

        // The account row itself stays bare, the count sits on the folders, as in Outlook.
        Assert.False(target[1].ShowUnread);
    }

    [Fact]
    public void A_known_folder_takes_its_roles_icon_and_a_custom_one_the_plain_folder()
    {
        var target = new ObservableCollection<SidebarItem>();
        Sync(target, [
            Account("a", folders:
            [
                Folder("inbox", "Postvak IN", SidebarFolderRole.Inbox),
                Folder("sent", "Verzonden items", SidebarFolderRole.Sent),
                // A custom folder, and one whose role we recognise but draw no icon for.
                Folder("klanten", "Klanten"),
                Folder("flagged", "Flagged", SidebarFolderRole.Other),
            ]),
        ]);

        // Localised names throughout: the icon must come from the role, or every language but
        // English gets the plain folder.
        Assert.Equal(
            ["inbox-glyph", "sent-glyph", "folder", "folder"],
            target[1].Children.Select(c => c.Glyph));
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
            [SidebarTree.AllInboxesTag, "acct:a", "acct:c", SidebarTree.AddAccountTag],
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
            [SidebarTree.AllInboxesTag, "acct:c", "acct:a", "acct:b", SidebarTree.AddAccountTag],
            target.Select(i => i.Tag));
        Assert.Same(c, target[1]);
        Assert.Same(a, target[2]);
    }
}
