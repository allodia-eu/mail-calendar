// Changing the folder tree from the pane (docs/folder-pane.md, "Changing the tree"): each row's
// menu and what its items ask, dragging a folder onto a folder or its account's row, dropping mail
// on a folder, and the notice a refused change leaves. Split out of MainWindow.Sidebar.cs, which
// draws the tree; this is what the user does to it.
//
// What a row offers and what a drop is taken for are FolderActions', pure and tested; this half
// only finds the row under the pointer and asks.

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.ApplicationModel.DataTransfer;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    /// <summary>The row's menu, built for the row the pointer or focus is on (rule 23).</summary>
    private MenuFlyout? FolderMenu(SidebarItem item)
    {
        var actions = FolderActions.Menu(item);
        if (actions.Count == 0)
        {
            return null;
        }
        var flyout = new MenuFlyout();
        foreach (var action in actions)
        {
            var entry = new MenuFlyoutItem { Text = MenuText(action) };
            entry.Click += async (_, _) => await RunFolderActionAsync(action, item);
            flyout.Items.Add(entry);
        }
        return flyout;
    }

    private static string MenuText(FolderMenuAction action) => action switch
    {
        FolderMenuAction.NewFolder => L10n.FolderActionNew(),
        FolderMenuAction.Rename => L10n.FolderActionRename(),
        FolderMenuAction.MoveTo => L10n.FolderActionMove(),
        FolderMenuAction.Delete => L10n.FolderActionDelete(),
        _ => L10n.ActionRemoveAccount(),
    };

    private async Task RunFolderActionAsync(FolderMenuAction action, SidebarItem item)
    {
        // An account row names its own account; a folder row, the account whose tree it is in.
        var account = item.AccountId ?? item.OwnerAccountId;
        if (account is null)
        {
            return;
        }
        var key = item.AccountId is null ? item.Tag : null;
        switch (action)
        {
            case FolderMenuAction.NewFolder:
                await NewFolderAsync(account, key);
                break;
            case FolderMenuAction.Rename when key is not null:
                await RenameFolderAsync(account, key, item.Content);
                break;
            case FolderMenuAction.MoveTo when key is not null:
                await MoveFolderAsync(account, key, item.Content);
                break;
            case FolderMenuAction.Delete when key is not null:
                await DeleteFolderAsync(account, key, item.Content, item.InTrash);
                break;
            case FolderMenuAction.RemoveAccount:
                await ConfirmRemoveAccountAsync(account, item.Content);
                break;
        }
    }

    private async Task NewFolderAsync(string account, string? parent)
    {
        var name = await FolderDialogs.AskNameAsync(
            Content.XamlRoot,
            L10n.FolderNewTitle(),
            L10n.ActionCreate(),
            string.Empty,
            typed => Model.CheckFolderName(account, parent, typed, null));
        if (name is not null)
        {
            Model.CreateFolder(account, parent, name);
        }
    }

    private async Task RenameFolderAsync(string account, string key, string current)
    {
        var parent = Model.FoldersOf(account).FirstOrDefault(f => f.Key == key)?.Parent;
        var name = await FolderDialogs.AskNameAsync(
            Content.XamlRoot,
            L10n.FolderRenameTitle(),
            L10n.ActionSave(),
            current,
            typed => Model.CheckFolderName(account, parent, typed, key));
        if (name is not null && name != current)
        {
            Model.RenameFolder(account, key, name);
        }
    }

    private async Task MoveFolderAsync(string account, string key, string name)
    {
        var targets = FolderActions.MoveTargets(Model.FoldersOf(account), key, L10n.FolderMoveTopLevel());
        var target = await FolderDialogs.AskMoveTargetAsync(Content.XamlRoot, L10n.FolderMoveTitle(name), targets);
        if (target is not null)
        {
            Model.MoveFolder(account, key, target.Key);
        }
    }

    private async Task DeleteFolderAsync(string account, string key, string name, bool inTrash)
    {
        var question = FolderActions.Delete(inTrash, name, new DeleteWords(
            L10n.FolderDeleteTitle,
            L10n.FolderDeleteMessage(),
            L10n.ActionMoveToTrash(),
            L10n.FolderDeletePermanentTitle,
            L10n.FolderDeletePermanentMessage(),
            L10n.ActionDeletePermanently()));
        var result = await DialogHelper.ConfirmAsync(Content.XamlRoot, question.Title, question.Message, question.Confirm);
        if (result == ContentDialogResult.Primary)
        {
            Model.DeleteFolder(account, key);
        }
    }

    /// <summary>
    /// A folder row starting to drag: only one the core lets move (rule 22). The payload stays in
    /// process, the package carries only the format that says a drag of ours is under the pointer.
    /// </summary>
    private void OnSidebarDragStarting(UIElement sender, DragStartingEventArgs args)
    {
        if (sender is not FrameworkElement { DataContext: SidebarItem { OwnerAccountId: { } owner, Editable: true, IsPending: false } row })
        {
            args.Cancel = true;
            return;
        }
        FolderActions.Dragged = new FolderDrag(owner, row.Tag);
        args.Data.SetData(FolderActions.DragFormat, row.Tag);
        args.Data.RequestedOperation = DataPackageOperation.Move;
    }

    private void OnSidebarDropCompleted(UIElement sender, DropCompletedEventArgs args) =>
        FolderActions.Dragged = null;

    /// <summary>Says over each row whether the drag in hand would land there.</summary>
    private void OnNavDragOver(object sender, DragEventArgs e)
    {
        if (DropTarget(e) is not { } row)
        {
            return;
        }
        e.AcceptedOperation = Takes(row) ? DataPackageOperation.Move : DataPackageOperation.None;
        e.Handled = true;
    }

    /// <summary>Moves what was dropped, when the row takes it.</summary>
    private void OnNavDrop(object sender, DragEventArgs e)
    {
        if (DropTarget(e) is not { } row)
        {
            return;
        }
        e.Handled = true;
        if (!Takes(row))
        {
            return;
        }
        var account = row.AccountId ?? row.OwnerAccountId!;
        var targetKey = row.AccountId is null ? row.Tag : null;
        switch (FolderActions.Dragged)
        {
            case FolderDrag folder:
                Model.MoveFolder(account, folder.Key, targetKey);
                break;
            case MessageDrag messages when targetKey is not null:
                Model.MoveMessagesToFolder(messages.Rows, account, targetKey);
                break;
        }
        FolderActions.Dragged = null;
    }

    /// <summary>The account or folder row under a drag of ours, or <c>null</c>: a drag from
    /// elsewhere (a file) or over any other row is none of this code's business.</summary>
    private static SidebarItem? DropTarget(DragEventArgs e) =>
        e.DataView.Contains(FolderActions.DragFormat)
        && e.OriginalSource is DependencyObject source
        && Ancestor<NavigationViewItem>(source) is { DataContext: SidebarItem row }
        && (row.AccountId is not null || row.OwnerAccountId is not null)
            ? row
            : null;

    private bool Takes(SidebarItem row)
    {
        var account = row.AccountId ?? row.OwnerAccountId;
        if (account is null || row.IsPending)
        {
            return false;
        }
        var targetKey = row.AccountId is null ? row.Tag : null;
        return FolderActions.Dragged switch
        {
            FolderDrag folder => FolderActions.TakesFolder(
                Model.FoldersOf(folder.Account), folder, account, targetKey, row.AcceptsFolders),
            MessageDrag messages => targetKey is not null
                && FolderActions.TakesMessages(messages, account, row.AcceptsMessages),
            _ => false,
        };
    }

    /// <summary>The notice bar's close: the core forgets the refusal (rule 28).</summary>
    private void OnDismissFolderNotice(InfoBar sender, object args) => Model.DismissFolderNotice();
}
