// What the folder pane offers and accepts when the user changes the tree (docs/folder-pane.md,
// "Changing the tree"): which items a row's menu carries, where Move to… may send a folder, which
// drops a row takes, and the words each dialog and the notice say.
//
// WinUI-free AND L10n-free on purpose, the same seam SidebarTree and EmptyMailboxLine use: the
// words arrive as parameters, so Mailcal.Tests can link this file and fail on the rules. Every
// eligibility here is the core's flag copied off the row (rule 22); what is decided locally is
// only what the core cannot see, which row the pointer is over and what is being dragged.

using System;
using System.Collections.Generic;
using System.Linq;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>One item of a folder or account row's menu, in the order the menu shows them.</summary>
internal enum FolderMenuAction
{
    NewFolder,
    Rename,
    MoveTo,
    Delete,
    RemoveAccount,
}

/// <summary>One destination Move to… offers: a folder key, or <c>null</c> for the top level.</summary>
internal sealed record MoveTarget(string? Key, string Label);

/// <summary>A folder being dragged, by its account and key together (rule 14).</summary>
internal sealed record FolderDrag(string Account, string Key);

/// <summary>Messages being dragged, in the shape the core's batched intent takes.</summary>
internal sealed record MessageDrag(SelectedRow[] Rows);

/// <summary>What a delete confirmation says, worded by where the folder is (rule 26).</summary>
internal sealed record DeleteQuestion(string Title, string Message, string Confirm);

/// <summary>The words the delete confirmations are made of.</summary>
internal sealed record DeleteWords(
    Func<string, string> Title,
    string Message,
    string Confirm,
    Func<string, string> PermanentTitle,
    string PermanentMessage,
    string PermanentConfirm);

/// <summary>The line under a name field for each answer <c>check_folder_name</c> can give.</summary>
internal sealed record FolderNameWords(
    string Empty,
    string Surrounded,
    string Control,
    string Separator,
    string Taken);

internal static class FolderActions
{
    /// <summary>The custom data format a drag inside this app carries. The payload itself stays in
    /// process (<see cref="Dragged"/>): no other app can drop a folder or a message here.</summary>
    public const string DragFormat = "Allodia.Mailcal.Drag";

    /// <summary>What is being dragged right now, set when a drag starts and cleared when it ends.</summary>
    public static object? Dragged { get; set; }

    /// <summary>
    /// The menu a row carries, or nothing: an account row offers New folder where its folders can
    /// be changed and always Remove account; a folder row offers what its flags allow, and a
    /// pending one nothing at all (rules 22, 23 and 27).
    /// </summary>
    public static IReadOnlyList<FolderMenuAction> Menu(SidebarItem row)
    {
        var items = new List<FolderMenuAction>();
        if (row.AccountId is not null)
        {
            if (row.AcceptsFolders)
            {
                items.Add(FolderMenuAction.NewFolder);
            }
            items.Add(FolderMenuAction.RemoveAccount);
            return items;
        }
        if (row.OwnerAccountId is null || row.IsPending)
        {
            return items;
        }
        if (row.AcceptsFolders)
        {
            items.Add(FolderMenuAction.NewFolder);
        }
        if (row.Editable)
        {
            items.Add(FolderMenuAction.Rename);
            items.Add(FolderMenuAction.MoveTo);
            items.Add(FolderMenuAction.Delete);
        }
        return items;
    }

    /// <summary>
    /// Where Move to… may send the folder <paramref name="key"/>: the top level first, then every
    /// folder of the account that takes folders, named with the folders it sits in, leaving out the
    /// folder itself and everything inside it (rule 24).
    /// </summary>
    public static IReadOnlyList<MoveTarget> MoveTargets(
        IReadOnlyList<FolderItem> folders, string key, string topLevel)
    {
        var targets = new List<MoveTarget> { new(null, topLevel) };
        foreach (var folder in folders)
        {
            if (folder.Key is { } candidate
                && folder.AcceptsFolders
                && !IsInside(folders, candidate, key))
            {
                targets.Add(new MoveTarget(candidate, Path(folders, folder)));
            }
        }
        return targets;
    }

    /// <summary>
    /// Whether a folder dropped on a row moves: only within its account, only onto a row that takes
    /// folders, never into itself or anything inside it, and never where it already is.
    /// <paramref name="targetKey"/> is <c>null</c> for the account's own row, the top of its tree.
    /// </summary>
    public static bool TakesFolder(
        IReadOnlyList<FolderItem> folders,
        FolderDrag drag,
        string targetAccount,
        string? targetKey,
        bool targetAcceptsFolders)
    {
        if (drag.Account != targetAccount || !targetAcceptsFolders)
        {
            return false;
        }
        var parent = folders.FirstOrDefault(f => f.Key == drag.Key)?.Parent;
        if (targetKey is null)
        {
            return parent is not null;
        }
        return targetKey != parent && !IsInside(folders, targetKey, drag.Key);
    }

    /// <summary>
    /// Whether dropped messages move into a folder: one that takes mail, and only when every row
    /// is of the folder's own account, since mail never crosses accounts (rule 24).
    /// </summary>
    public static bool TakesMessages(MessageDrag drag, string targetAccount, bool targetAcceptsMessages) =>
        targetAcceptsMessages
        && drag.Rows.Length > 0
        && drag.Rows.All(row => AccountOf(row) == targetAccount);

    /// <summary>Whether <paramref name="key"/> is <paramref name="ancestor"/> or lies inside it.</summary>
    public static bool IsInside(IReadOnlyList<FolderItem> folders, string key, string ancestor)
    {
        string? at = key;
        // Bounded by the list's length, so a tree that loops cannot hang the walk.
        for (var step = 0; at is not null && step <= folders.Count; step++)
        {
            if (at == ancestor)
            {
                return true;
            }
            var current = at;
            at = folders.FirstOrDefault(f => f.Key == current)?.Parent;
        }
        return false;
    }

    /// <summary>A folder's name with the folders it sits in, "Clients / Acme", for a flat list.</summary>
    public static string Path(IReadOnlyList<FolderItem> folders, FolderItem folder)
    {
        var parts = new List<string> { folder.Name };
        var parent = folder.Parent;
        while (parent is not null && parts.Count <= folders.Count)
        {
            var current = parent;
            var ancestor = folders.FirstOrDefault(f => f.Key == current);
            if (ancestor is null)
            {
                break;
            }
            parts.Add(ancestor.Name);
            parent = ancestor.Parent;
        }
        parts.Reverse();
        return string.Join(" / ", parts);
    }

    /// <summary>Whether a name dialog may confirm, and the line under its field (rule 25).</summary>
    public static (bool CanConfirm, string Line) NameState(FolderNameCheck check, FolderNameWords words) =>
        check switch
        {
            FolderNameCheck.Valid => (true, string.Empty),
            FolderNameCheck.Empty => (false, words.Empty),
            FolderNameCheck.Surrounded => (false, words.Surrounded),
            FolderNameCheck.Control => (false, words.Control),
            FolderNameCheck.Separator => (false, words.Separator),
            FolderNameCheck.Taken => (false, words.Taken),
            // A later answer this build does not know: refuse rather than send a name the core
            // has already said something about.
            _ => (false, string.Empty),
        };

    /// <summary>The delete confirmation for a folder called <paramref name="name"/>: into Trash
    /// outside it, for good inside it (rule 26).</summary>
    public static DeleteQuestion Delete(bool inTrash, string name, DeleteWords words) =>
        inTrash
            ? new DeleteQuestion(words.PermanentTitle(name), words.PermanentMessage, words.PermanentConfirm)
            : new DeleteQuestion(words.Title(name), words.Message, words.Confirm);

    /// <summary>The notice's sentence, or the empty string when there is no notice (rule 28).</summary>
    public static string NoticeText(
        FolderNotice? notice, Func<string, string> changed, Func<string, string> refused) =>
        notice switch
        {
            null => string.Empty,
            { Problem: FolderProblem.ChangedElsewhere } => changed(notice.Folder),
            _ => refused(notice.Folder),
        };

    private static string? AccountOf(SelectedRow row) => row switch
    {
        SelectedRow.Message message => message.Account,
        SelectedRow.Thread thread => thread.Account,
        _ => null,
    };
}
