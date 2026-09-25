// Changing the folder tree from the pane (docs/folder-pane.md, "Changing the tree", rules 22 to 28):
// what each row's menu carries, where Move to… may send a folder, which drops land, and the words
// the dialogs and the notice use.
//
// Every one of these fails silently on screen. A folder offered a Delete it may not have is a
// click the core ignores; a Move to… that lists the folder's own subfolder moves it nowhere; a drop
// taken across accounts moves nothing and looks as if it did. None of it is reachable from a
// screenshot, and the WinUI half cannot be linked into this assembly.

using System.Collections.ObjectModel;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class FolderActionsTests
{
    private const string Account = "acct-1";

    private static readonly FolderNameWords NameWords =
        new("empty", "surrounded", "control", "separator", "taken");

    private static FolderItem Folder(
        string key,
        string name,
        string? parent = null,
        bool editable = true,
        bool acceptsFolders = true,
        bool pending = false,
        SidebarFolderRole role = SidebarFolderRole.None) =>
        new()
        {
            Key = key,
            Name = name,
            Parent = parent,
            Role = role,
            Editable = editable,
            AcceptsFolders = acceptsFolders,
            AcceptsMessages = !pending,
            Pending = pending,
        };

    /// <summary>Inbox, then `work` holding `clients` holding `acme`, then `bills` at the top.</summary>
    private static List<FolderItem> Tree() =>
        [
            Folder("inbox", "Inbox", editable: false, role: SidebarFolderRole.Inbox),
            Folder("work", "Work"),
            Folder("clients", "Clients", parent: "work"),
            Folder("acme", "Acme", parent: "clients"),
            Folder("bills", "Bills"),
        ];

    private static SidebarItem FolderRow(bool editable, bool acceptsFolders, bool pending = false) =>
        new()
        {
            Tag = "work",
            OwnerAccountId = Account,
            Editable = editable,
            AcceptsFolders = acceptsFolders,
            IsPending = pending,
        };

    [Fact]
    public void AFolderRowOffersExactlyWhatItsFlagsAllow()
    {
        Assert.Equal(
            new[] { FolderMenuAction.NewFolder, FolderMenuAction.Rename, FolderMenuAction.MoveTo, FolderMenuAction.Delete },
            FolderActions.Menu(FolderRow(editable: true, acceptsFolders: true)));
        // A role folder (the Inbox) takes subfolders and is named and placed by the app.
        Assert.Equal(
            new[] { FolderMenuAction.NewFolder },
            FolderActions.Menu(FolderRow(editable: false, acceptsFolders: true)));
        // A folder inside Trash can be restored or removed, and takes no new subfolders.
        Assert.Equal(
            new[] { FolderMenuAction.Rename, FolderMenuAction.MoveTo, FolderMenuAction.Delete },
            FolderActions.Menu(FolderRow(editable: true, acceptsFolders: false)));
        // Junk, Drafts: nothing, so no menu at all.
        Assert.Empty(FolderActions.Menu(FolderRow(editable: false, acceptsFolders: false)));
    }

    [Fact]
    public void APendingFolderOffersNothingWhateverItsFlagsSay()
    {
        // Its key may still move on the server, so nothing may be built on it (rule 27).
        Assert.Empty(FolderActions.Menu(FolderRow(editable: true, acceptsFolders: true, pending: true)));
    }

    [Fact]
    public void AnAccountRowOffersNewFolderOnlyWhereItsFoldersCanChange()
    {
        var managed = new SidebarItem { Tag = "acct:a", AccountId = "a", AcceptsFolders = true };
        var fixedTree = new SidebarItem { Tag = "acct:b", AccountId = "b", AcceptsFolders = false };
        Assert.Equal(
            new[] { FolderMenuAction.NewFolder, FolderMenuAction.RemoveAccount },
            FolderActions.Menu(managed));
        Assert.Equal(new[] { FolderMenuAction.RemoveAccount }, FolderActions.Menu(fixedTree));
    }

    [Fact]
    public void TheGroupAndTheSyntheticRowsCarryNoMenu()
    {
        Assert.Empty(FolderActions.Menu(new SidebarItem { Tag = SidebarTree.AllAccountsTag, IsGroup = true }));
        Assert.Empty(FolderActions.Menu(new SidebarItem { Tag = SidebarTree.AddAccountTag }));
    }

    [Fact]
    public void MoveToListsTheTopLevelFirstAndNeverTheFolderOrWhatIsInsideIt()
    {
        var targets = FolderActions.MoveTargets(Tree(), "clients", "Top level");

        Assert.Equal((string?)null, targets[0].Key);
        Assert.Equal("Top level", targets[0].Label);
        var keys = targets.Skip(1).Select(t => t.Key).ToList();
        Assert.DoesNotContain("clients", keys);
        Assert.DoesNotContain("acme", keys);
        Assert.Contains("work", keys);
        Assert.Contains("bills", keys);
    }

    [Fact]
    public void MoveToNamesEachFolderWithTheFoldersItSitsIn()
    {
        var targets = FolderActions.MoveTargets(Tree(), "bills", "Top level");
        Assert.Contains(targets, t => t.Key == "acme" && t.Label == "Work / Clients / Acme");
    }

    [Fact]
    public void MoveToLeavesOutAFolderThatTakesNoSubfolders()
    {
        var tree = Tree();
        tree.Add(Folder("trash", "Trash", editable: false, acceptsFolders: false, role: SidebarFolderRole.Trash));
        Assert.DoesNotContain(FolderActions.MoveTargets(tree, "bills", "Top level"), t => t.Key == "trash");
    }

    [Fact]
    public void AFolderDropLandsOnlyWhereItCanGoWithinItsAccount()
    {
        var tree = Tree();
        var drag = new FolderDrag(Account, "clients");

        Assert.True(FolderActions.TakesFolder(tree, drag, Account, "bills", targetAcceptsFolders: true));
        Assert.True(FolderActions.TakesFolder(tree, drag, Account, null, targetAcceptsFolders: true),
            "its account's row moves it to the top");
        Assert.False(FolderActions.TakesFolder(tree, drag, "acct-2", "bills", targetAcceptsFolders: true),
            "a folder never crosses accounts");
        Assert.False(FolderActions.TakesFolder(tree, drag, Account, "acme", targetAcceptsFolders: true),
            "a folder cannot move inside itself");
        Assert.False(FolderActions.TakesFolder(tree, drag, Account, "clients", targetAcceptsFolders: true));
        Assert.False(FolderActions.TakesFolder(tree, drag, Account, "work", targetAcceptsFolders: true),
            "dropped back where it already is, it does not move");
        Assert.False(FolderActions.TakesFolder(tree, drag, Account, "bills", targetAcceptsFolders: false));
    }

    [Fact]
    public void ATopLevelFolderDroppedOnItsOwnAccountIsNotAMove()
    {
        Assert.False(FolderActions.TakesFolder(
            Tree(), new FolderDrag(Account, "bills"), Account, null, targetAcceptsFolders: true));
    }

    [Fact]
    public void MessagesLandOnlyOnAFolderOfTheirOwnAccountThatTakesMail()
    {
        var mine = new MessageDrag(
            [new SelectedRow.Message(Account, "m1"), new SelectedRow.Thread(Account, "t1")]);
        var mixed = new MessageDrag(
            [new SelectedRow.Message(Account, "m1"), new SelectedRow.Message("acct-2", "m2")]);

        Assert.True(FolderActions.TakesMessages(mine, Account, targetAcceptsMessages: true));
        Assert.False(FolderActions.TakesMessages(mine, Account, targetAcceptsMessages: false),
            "Junk takes mail through a report, never a drop");
        Assert.False(FolderActions.TakesMessages(mine, "acct-2", targetAcceptsMessages: true));
        Assert.False(FolderActions.TakesMessages(mixed, Account, targetAcceptsMessages: true),
            "a selection spanning accounts would move only part of itself");
        Assert.False(FolderActions.TakesMessages(new MessageDrag([]), Account, targetAcceptsMessages: true));
    }

    [Fact]
    public void ATreeThatLoopsDoesNotHangTheWalk()
    {
        List<FolderItem> looped = [Folder("a", "A", parent: "b"), Folder("b", "B", parent: "a")];
        Assert.False(FolderActions.IsInside(looped, "a", "c"));
        Assert.True(FolderActions.IsInside(looped, "a", "b"));
    }

    [Fact]
    public void TheNameDialogConfirmsOnlyAValidNameAndSaysWhyOtherwise()
    {
        Assert.Equal((true, string.Empty), FolderActions.NameState(FolderNameCheck.Valid, NameWords));
        Assert.Equal((false, "empty"), FolderActions.NameState(FolderNameCheck.Empty, NameWords));
        Assert.Equal((false, "surrounded"), FolderActions.NameState(FolderNameCheck.Surrounded, NameWords));
        Assert.Equal((false, "control"), FolderActions.NameState(FolderNameCheck.Control, NameWords));
        Assert.Equal((false, "separator"), FolderActions.NameState(FolderNameCheck.Separator, NameWords));
        Assert.Equal((false, "taken"), FolderActions.NameState(FolderNameCheck.Taken, NameWords));
    }

    [Fact]
    public void TheDeleteQuestionSaysTrashOutsideTrashAndForGoodInsideIt()
    {
        var words = new DeleteWords(
            name => $"Delete {name}?", "into Trash", "Move to Trash",
            name => $"Delete {name} permanently?", "for good", "Delete permanently");

        var outside = FolderActions.Delete(inTrash: false, "Bills", words);
        var inside = FolderActions.Delete(inTrash: true, "Bills", words);

        Assert.Equal(new DeleteQuestion("Delete Bills?", "into Trash", "Move to Trash"), outside);
        Assert.Equal(new DeleteQuestion("Delete Bills permanently?", "for good", "Delete permanently"), inside);
    }

    [Fact]
    public void TheNoticeNamesTheFolderAndWhyTheChangeDidNotHappen()
    {
        static string Text(FolderNotice? notice) =>
            FolderActions.NoticeText(notice, name => $"changed:{name}", name => $"refused:{name}");

        Assert.Equal(string.Empty, Text(null));
        Assert.Equal("changed:Work", Text(new FolderNotice(Account, "Work", FolderAction.Rename, FolderProblem.ChangedElsewhere)));
        Assert.Equal("refused:Work", Text(new FolderNotice(Account, "Work", FolderAction.Delete, FolderProblem.Refused)));
    }

    [Fact]
    public void TheReconcileCopiesWhatEachRowOffersAndWhetherItWaits()
    {
        var target = new ObservableCollection<SidebarItem>();
        var account = new AccountItem
        {
            Id = Account,
            Email = "a@example.com",
            SendLabel = "a@example.com",
            Expanded = true,
            ManagesFolders = true,
            Folders =
            [
                Folder("work", "Work"),
                Folder("pending-folder:3", "Travel", pending: true, editable: false, acceptsFolders: false),
            ],
        };
        SidebarFixture.Sync(target, [account]);

        var accountRow = target.Single(i => i.AccountId == Account);
        Assert.True(accountRow.AcceptsFolders, "the account row carries the account's capability");
        var work = accountRow.Children.Single(i => i.Tag == "work");
        Assert.True(work.Editable && work.AcceptsFolders && work.AcceptsMessages);
        Assert.False(work.IsPending);
        Assert.Null(work.PendingTip);
        Assert.Equal(1.0, work.RowOpacity);
        var travel = accountRow.Children.Single(i => i.Tag == "pending-folder:3");
        Assert.True(travel.IsPending);
        Assert.Equal("Waiting to sync", travel.PendingLabel);
        Assert.Equal("Waiting to sync", travel.PendingTip);
        Assert.True(travel.RowOpacity < 1.0, "a pending row draws dimmed");
    }
}
