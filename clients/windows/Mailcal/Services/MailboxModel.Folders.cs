// Changing the folder tree (docs/folder-pane.md, "Changing the tree"): the outbound intents, the
// name check a dialog asks as the user types. A refused change is a notice (MailboxModel.Notices.cs).
//
// Its own partial, like MailboxModel.Outbox.cs. The decisions (which rows offer what, which drops
// are taken, the words) are FolderActions, pure and gated by Mailcal.Tests; this half is only the
// edge to the core and the property notifications the shell binds to.

using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>One account's folders as the pane has them, for Move to… and a drop's checks.</summary>
    internal IReadOnlyList<FolderItem> FoldersOf(string account) =>
        Accounts.FirstOrDefault(a => a.Id == account)?.Folders ?? [];

    /// <summary>
    /// Whether <paramref name="name"/> can be given to a folder inside <paramref name="parent"/>
    /// (<c>null</c>: the top level); <paramref name="renaming"/> is the folder being renamed, which
    /// may keep its own name. Asked on every keystroke of a name dialog (rule 25).
    /// </summary>
    internal FolderNameCheck CheckFolderName(string account, string? parent, string name, string? renaming) =>
        _app?.CheckFolderName(account, parent, name, renaming) ?? FolderNameCheck.Empty;

    /// <summary>Makes a folder called <paramref name="name"/> inside <paramref name="parent"/>, or
    /// at the top of the account's tree.</summary>
    internal void CreateFolder(string account, string? parent, string name) =>
        DispatchFolders(new FolderIntent.Create(account, parent, name));

    /// <summary>Renames a folder where it is.</summary>
    internal void RenameFolder(string account, string key, string name) =>
        DispatchFolders(new FolderIntent.Rename(account, key, name));

    /// <summary>Moves a folder, with everything inside it, into <paramref name="parent"/> or to the top.</summary>
    internal void MoveFolder(string account, string key, string? parent) =>
        DispatchFolders(new FolderIntent.Move(account, key, parent));

    /// <summary>Deletes a folder: into Trash, or for good from inside it (rule 26).</summary>
    internal void DeleteFolder(string account, string key) =>
        DispatchFolders(new FolderIntent.Delete(account, key));

    /// <summary>
    /// Moves dropped messages into a folder. The reading pane lets go of a message leaving the list,
    /// as a batch archive does.
    /// </summary>
    internal void MoveMessagesToFolder(SelectedRow[] rows, string account, string key)
    {
        var closesReading = HoldsOpenMessage(rows);
        DispatchFolders(new FolderIntent.MoveMessages(rows, account, key));
        if (closesReading)
        {
            CloseReading();
        }
    }

    private void DispatchFolders(FolderIntent intent) => _app?.Dispatch(new Intent.Folders(intent));

    private bool HoldsOpenMessage(SelectedRow[] rows) =>
        OpenedMessage is { } opened
        && rows.Any(row => row switch
        {
            SelectedRow.Message message => message.Account == opened.Account && message.Key == opened.Key,
            SelectedRow.Thread thread => Rows.Any(r =>
                r.IsThread
                && r.Account == thread.Account
                && r.Key == thread.ThreadId
                && r.Messages.Any(m => m.Account == opened.Account && m.Key == opened.Key)),
            _ => false,
        });
}
