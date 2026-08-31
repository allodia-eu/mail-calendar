// What a folder is CALLED on screen: the app's own word for a known folder, the server's name for
// everything else (docs/folder-pane.md rule 12).
//
// Beside the projections rather than in the sidebar, because the sidebar is not the only place a
// folder is named, the list header, the sync-settings folder list and the account settings dialog
// all show one too, and a fixed label that appears in three of those four reads as a bug in the
// fourth. Every site calls through here.

using Allodia.Mailcal.ViewModels;

namespace Allodia.Mailcal.Services;

/// <summary>Turns a folder's role and server name into the label the user reads.</summary>
internal static class FolderLabel
{
    /// <summary>
    /// The app's own word for a role-bearing folder; <paramref name="serverName"/> for an ordinary
    /// one.
    /// </summary>
    /// <remarks>
    /// <para>
    /// The server's own name for a special folder is not a name the user chose, it is whatever
    /// their provider happens to store, and it arrives in whatever language and casing that
    /// provider likes: <c>INBOX</c> shouting in capitals (the one name IMAP actually mandates),
    /// <c>Deleted Items</c> from Exchange, <c>[Gmail]/Sent Mail</c>. Naming those ourselves is what
    /// every mail client does, and it is also what makes the folder list follow the app's language
    /// instead of the server's.
    /// </para>
    /// <para>
    /// <see cref="SidebarFolderRole.Other"/> deliberately keeps the server name: the core collapses
    /// flagged, important and all-mail into that one value, so there is no single honest word for
    /// it, and inventing one would rename three different folders to the same thing.
    /// </para>
    /// </remarks>
    public static string For(SidebarFolderRole role, string serverName) => role switch
    {
        SidebarFolderRole.Inbox => L10n.FolderInbox(),
        SidebarFolderRole.Drafts => L10n.FolderDrafts(),
        SidebarFolderRole.Sent => L10n.FolderSent(),
        SidebarFolderRole.Archive => L10n.FolderArchive(),
        SidebarFolderRole.Junk => L10n.FolderJunk(),
        SidebarFolderRole.Trash => L10n.FolderTrash(),
        _ => serverName,
    };

    /// <summary>The same, from the core's own role type.</summary>
    public static string For(uniffi.mailcal_bindings.FolderRole? role, string serverName) =>
        For(MailboxModel.ToSidebarRole(role), serverName);
}
