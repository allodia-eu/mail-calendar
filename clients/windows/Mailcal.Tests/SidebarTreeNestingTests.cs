// How the folder pane NESTS on Windows: a folder filed inside another becomes a child of that
// folder's entry, so the framework draws the chevron, the indent and the hiding itself, where the
// three panes that draw a flat list read FolderRow.visible instead (docs/folder-pane.md, rule 19).
//
// Its own suite because the rules above it in SidebarTreeTests are about the pane's top level (the
// group, the accounts, the Outbox, the badges) and these are about what hangs underneath, and
// because the two together no longer fit one file.
//
// Two of these fail silently in the other direction: the tree looks right in a screenshot taken a
// second later, and what catches them is asserting WHEN a row was expanded relative to the rows
// appearing inside it.

using System.Collections.ObjectModel;
using Allodia.Mailcal.ViewModels;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SidebarTreeNestingTests
{
    // A folder filed inside another becomes a child of that folder's entry, not of the account's.
    // This pane nests rows natively, so the framework draws the chevron, the indent and the
    // hiding; the three panes that draw a flat list read FolderRow.visible instead
    // (docs/folder-pane.md).
    [Fact]
    public void FolderInsideFolderIsNestedUnderIt()
    {
        var target = new ObservableCollection<SidebarItem>();
        SidebarFixture.Sync(target, [SidebarFixture.Account("a", folders: SidebarFixture.Tree())]);

        var account = target.Single(item => item.AccountId == "a");
        var outer = Assert.Single(account.Children);
        Assert.Equal("outer", outer.Tag);
        var inner = Assert.Single(outer.Children);
        Assert.Equal("inner", inner.Tag);
        Assert.Equal("deep", Assert.Single(inner.Children).Tag);

        // Every level carries its account, because a folder key names a mailbox only together
        // with one (rule 14).
        Assert.Equal("a", inner.OwnerAccountId);
        Assert.Equal("a", Assert.Single(inner.Children).OwnerAccountId);
    }

    // A folder holding nothing but mail gets no children, which is what keeps the chevron off it:
    // the framework draws one only for an item that has some.
    [Fact]
    public void AFolderHoldingNoFoldersHasNoChildren()
    {
        var target = new ObservableCollection<SidebarItem>();
        SidebarFixture.Sync(target, [SidebarFixture.Account("a", folders: SidebarFixture.Tree())]);

        var deep = target
            .Single(item => item.AccountId == "a")
            .Children.Single()
            .Children.Single()
            .Children.Single();
        Assert.Empty(deep.Children);
    }

    // Expansion is the core's, mirrored here and never invented: a shut folder comes back shut,
    // and applying the core's own value must not report itself as a user's chevron click.
    [Fact]
    public void FolderExpansionMirrorsTheCoreWithoutEchoingBack()
    {
        var toggled = new List<SidebarItem>();
        var target = new ObservableCollection<SidebarItem>();
        SidebarFixture.Sync(
            target,
            [SidebarFixture.Account("a", folders: SidebarFixture.Tree(innerExpanded: false))],
            onExpanded: toggled.Add);

        var outer = target.Single(item => item.AccountId == "a").Children.Single();
        Assert.True(outer.IsExpanded);
        Assert.False(outer.Children.Single().IsExpanded);
        Assert.Empty(toggled);
    }

    // A refresh reuses the entry a nested folder already has. The flat lookup could not find one:
    // a nested folder is a child of its parent's entry, not of the account's, so a miss would mint
    // a second entry and cost the framework a container it had already realised.
    [Fact]
    public void ARefreshKeepsTheEntriesANestedTreeAlreadyHas()
    {
        var target = new ObservableCollection<SidebarItem>();
        var account = SidebarFixture.Account("a", folders: SidebarFixture.Tree());
        SidebarFixture.Sync(target, [account]);
        var before = target.Single(item => item.AccountId == "a").Children.Single().Children.Single();

        SidebarFixture.Sync(target, [account]);

        var after = target.Single(item => item.AccountId == "a").Children.Single().Children.Single();
        Assert.Same(before, after);
    }

    // A folder that stops holding folders loses the rows that were under it, rather than keeping
    // them on screen beneath a parent that no longer names them.
    [Fact]
    public void AFolderThatEmptiesOutDropsWhatWasUnderIt()
    {
        var target = new ObservableCollection<SidebarItem>();
        SidebarFixture.Sync(target, [SidebarFixture.Account("a", folders: SidebarFixture.Tree())]);

        SidebarFixture.Sync(
            target,
            [SidebarFixture.Account("a", folders: [SidebarFixture.Folder("outer", "Outer")])]);

        var outer = target.Single(item => item.AccountId == "a").Children.Single();
        Assert.Equal("outer", outer.Tag);
        Assert.Empty(outer.Children);
    }
    // A folder discovered to hold folders is expanded only once those folders are attached to it.
    // NavigationViewItem applies an expansion against the children it has at that moment, so a row
    // whose IsExpanded turns true while its Children are still empty draws the open chevron over a
    // child host it never showed: the user gets a row that claims to be open, holding nothing, and
    // has to click twice to shut and reopen it. It is reachable only while a sync is still running,
    // because that is when a parent already on screen gains its first child.
    [Fact]
    public void AFolderIsExpandedOnlyOnceWhatIsInsideItIsAttached()
    {
        var target = new ObservableCollection<SidebarItem>();
        // The sync has not found the subfolder yet: one flat row, holding nothing.
        SidebarFixture.Sync(
            target,
            [SidebarFixture.Account("a", folders: [SidebarFixture.Folder("outer", "Outer")])]);
        var outer = target.Single(item => item.AccountId == "a").Children.Single();
        Assert.False(outer.IsExpanded);
        Assert.Empty(outer.Children);

        var childrenWhenExpanded = -1;
        outer.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(SidebarItem.IsExpanded) && outer.IsExpanded)
            {
                childrenWhenExpanded = outer.Children.Count;
            }
        };

        // The next refresh finds what is inside it.
        SidebarFixture.Sync(
            target,
            [
                SidebarFixture.Account("a", folders:
                [
                    SidebarFixture.Folder("outer", "Outer", hasChildren: true, expanded: true),
                    SidebarFixture.Folder("inner", "Inner", parent: "outer"),
                ]),
            ]);

        Assert.True(outer.IsExpanded);
        Assert.Equal("inner", Assert.Single(outer.Children).Tag);
        Assert.Equal(1, childrenWhenExpanded);
    }
    // A row already on screen must never be left holding nothing, because a row the framework has
    // realised empty can be told it is open and then show nothing at all. The calendar and
    // contacts destinations draw no mail tree, and the way they do it is by shutting each account
    // row rather than emptying it, so the tree a returning row reopens was attached the whole
    // time. This watches the collection itself: the count may never reach zero.
    [Fact]
    public void LeavingMailNeverEmptiesAnAccountRow()
    {
        var target = new ObservableCollection<SidebarItem>();
        var account = SidebarFixture.Account("a", folders: SidebarFixture.Tree());
        SidebarFixture.Sync(target, [account]);
        var row = target.Single(item => item.AccountId == "a");
        Assert.Single(row.Children);

        var everEmptied = false;
        row.Children.CollectionChanged += (_, _) => everEmptied |= row.Children.Count == 0;

        SidebarFixture.Sync(target, [account], showFolders: false);
        Assert.False(row.IsExpanded);
        Assert.Single(row.Children);

        SidebarFixture.Sync(target, [account]);
        Assert.True(row.IsExpanded);
        Assert.Single(row.Children);
        Assert.False(everEmptied);
    }
}
