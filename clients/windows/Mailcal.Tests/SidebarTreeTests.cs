// The folder pane's rules on Windows (docs/folder-pane.md), as the bound accordion's shape.
//
// They are the kind that fail silently: an expansion driven off the selection looks perfectly
// reasonable until you notice the tree emptying itself, an unread badge that never updates looks
// like a folder with no new mail, and a heading that navigates looks like one more account row
// until it claims a scope the core has no Scope for.
//
// What the same reconcile COSTS is SidebarTreeCostTests, and the fixtures both use are
// SidebarFixture.

using System.Collections.ObjectModel;
using Allodia.Mailcal.ViewModels;
using Xunit;

using static Allodia.Mailcal.Tests.SidebarFixture;

namespace Allodia.Mailcal.Tests;

public class SidebarTreeTests
{
    [Fact]
    public void Builds_the_all_accounts_group_then_accounts_then_add_account()
    {
        var target = new ObservableCollection<SidebarItem>();
        Sync(target, Accounts("a", "b"));

        Assert.Equal(
            [SidebarTree.AllAccountsTag, "acct:a", "acct:b", SidebarTree.AddAccountTag],
            target.Select(i => i.Tag));
        Assert.Equal("All Accounts", target[0].Content);
        Assert.Equal("a@example.com", target[1].Content);
        // Add account is an action, not a destination, so it must never hold the selection.
        Assert.False(target[^1].SelectsOnInvoked);
        Assert.True(target[1].SelectsOnInvoked);
    }

    [Fact]
    public void The_unified_list_is_a_group_over_an_inbox_row_not_a_row_of_its_own()
    {
        // Rule 16 (docs/folder-pane.md): the unified scope is a tree like an account's, and the
        // destination is the Inbox *under* the heading. The group is the shape a unified Sent and
        // Drafts need; today it has the one child, because the core's unified scope reaches every
        // account's Inbox and nothing else.
        var target = new ObservableCollection<SidebarItem>();
        Sync(target, Accounts("a", "b"), unifiedUnread: 12);

        var group = target[0];
        Assert.True(group.IsGroup);
        Assert.Equal("All Accounts", group.Content);
        // Rule 17: the heading navigates nowhere. Activating it opens or shuts its tree, which is
        // why it must not take the selection an account row does.
        Assert.False(group.SelectsOnInvoked);
        // Rule 8, which the group is inside: the count belongs to the row beneath, never to the
        // heading directly above the identical number.
        Assert.False(group.ShowUnread);
        Assert.Equal(string.Empty, group.UnreadLabel);

        var inbox = Assert.Single(group.Children);
        Assert.Equal(SidebarTree.AllInboxesTag, inbox.Tag);
        Assert.Equal("Inbox", inbox.Content);
        Assert.Equal("inbox-glyph", inbox.Glyph);
        Assert.Equal(12u, inbox.Unread);
        Assert.True(inbox.SelectsOnInvoked);
        // It names no owning account: the unified scope belongs to none, and the shell reads the
        // tag rather than an account id to know what the click meant.
        Assert.Null(inbox.OwnerAccountId);
    }

    [Fact]
    public void A_shut_group_takes_the_unified_inbox_off_screen_like_a_shut_account()
    {
        var target = new ObservableCollection<SidebarItem>();
        var toggles = new List<bool>();
        void Refresh(bool expanded) => Sync(
            target,
            Accounts("a"),
            unifiedExpanded: expanded,
            onExpanded: i => toggles.Add(i.IsExpanded));

        Refresh(expanded: true);
        Assert.True(target[0].IsExpanded);

        Refresh(expanded: false);
        Assert.False(target[0].IsExpanded);
        // The row is still in the data, as a shut account's folders are: the framework draws it
        // or not off IsExpanded, so shutting costs no container work either way.
        Assert.Single(target[0].Children);
        // And the account beside it is untouched: the two trees are independent (rule 2).
        Assert.True(target[1].IsExpanded);

        // The core's own value coming back, five times over, is not five user actions.
        for (var i = 0; i < 5; i++)
        {
            Refresh(expanded: false);
        }
        Assert.Empty(toggles);
    }

    [Fact]
    public void The_groups_chevron_reaches_the_core_and_says_it_is_the_group()
    {
        // The shell dispatches SetUnifiedExpanded for the group and SetAccountExpanded for an
        // account, off the same callback, so what it reads to tell them apart is asserted here.
        var target = new ObservableCollection<SidebarItem>();
        var toggles = new List<(bool Group, string? Account, bool Expanded)>();
        Sync(
            target,
            Accounts("a"),
            onExpanded: i => toggles.Add((i.IsGroup, i.AccountId, i.IsExpanded)));

        target[0].IsExpanded = false;
        target[1].IsExpanded = false;

        Assert.Equal([(true, null, false), (false, "a", false)], toggles);
    }

    [Fact]
    public void Every_account_carries_its_own_folders_not_just_the_selected_one()
    {
        // The regression this replaced: the pane was fed the SELECTED account's folders alone, so
        // choosing the unified Inbox, or the account next to it, emptied the tree on screen.
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

        Sync(target, accounts, showFolders: true, onExpanded: i => toggles.Add((i.AccountId ?? i.Tag, i.IsExpanded)));
        Sync(target, accounts, showFolders: false, onExpanded: i => toggles.Add((i.AccountId ?? i.Tag, i.IsExpanded)));

        Assert.Empty(target[1].Children);
        Assert.False(target[1].IsExpanded);
        // The All Accounts group goes with them: the unified Inbox is a folder, and the calendar
        // is not what a folder pane is for.
        Assert.Empty(target[0].Children);
        Assert.False(target[0].IsExpanded);
        // Crucially, the core was never told the user shut anything, so coming back to mail
        // restores the tree rather than reopening a collapsed one.
        Assert.Empty(toggles);

        Sync(target, accounts, showFolders: true);
        Assert.True(target[1].IsExpanded);
        Assert.Equal(3, target[1].Children.Count);
        Assert.True(target[0].IsExpanded);
        Assert.Single(target[0].Children);
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
    public void Unread_counts_land_on_folders_and_on_the_unified_inbox_and_vanish_at_zero()
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
        Assert.Equal(548u, target[0].Children[0].Unread);
        Assert.True(target[0].Children[0].ShowUnread);

        // The account row itself stays bare, the count sits on the folders, as in Outlook, and
        // the All Accounts heading is bare for the same reason (rule 8).
        Assert.False(target[1].ShowUnread);
        Assert.False(target[0].ShowUnread);
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
    }}
