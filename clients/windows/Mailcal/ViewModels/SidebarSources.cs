// The two model collections the sidebar is built from. Their own file rather than a corner of
// RowViewModels.cs, for the reason RichComposeKind has one: that file binds Visibility and
// FontWeight, and this one has to compile into Mailcal.Tests, a plain net10.0 assembly, so
// SidebarTree's shaping rules can be pinned without a window.

namespace Allodia.Mailcal.ViewModels;

/// <summary>
/// A folder's special role, mirroring the core's <c>FolderRole</c> one-for-one.
/// </summary>
/// <remarks>
/// A mirror rather than the generated enum itself, for a reason the compiler enforces: the UniFFI
/// bindings are generated <c>internal</c> (they are compiled INTO each assembly rather than
/// referenced), so a public member here cannot name one. The mapping lives in
/// <c>MailboxModel.Projection</c>, on the side of the line where the generated types are visible.
/// <para>
/// <c>None</c> is a member rather than a nullable, because "no special role" is a real answer this
/// enum has to carry through a non-nullable projection.
/// </para>
/// </remarks>
public enum SidebarFolderRole
{
    /// <summary>An ordinary custom folder, and the synthetic "All Mail" head.</summary>
    None,

    /// <summary>The primary inbox.</summary>
    Inbox,

    /// <summary>Drafts, messages in progress.</summary>
    Drafts,

    /// <summary>Sent, copies of sent messages.</summary>
    Sent,

    /// <summary>Archive, long-term storage.</summary>
    Archive,

    /// <summary>Junk / Spam.</summary>
    Junk,

    /// <summary>Trash, recoverable deleted messages.</summary>
    Trash,

    /// <summary>A role-bearing folder we draw no distinct icon for (flagged, all, important).</summary>
    Other,
}

/// <summary>One sidebar folder: its provider key, display name, role, and unread count.</summary>
public sealed class FolderItem
{
    /// <summary>The mailbox's provider key (used to select it); null for "All Mail".</summary>
    public string? Key { get; init; }

    /// <summary>The folder's display name.</summary>
    public required string Name { get; init; }

    /// <summary>The folder's special role. Picks the row's icon, never a name heuristic, which
    /// would pick the wrong glyph in six of the seven shipped languages, and on any server whose
    /// folders were renamed.</summary>
    public SidebarFolderRole Role { get; init; }

    /// <summary>How many messages in the folder are unread, as the server counts them. <c>0</c>
    /// shows no badge, and deliberately covers both "nothing unread" and "this provider reports
    /// no count" (docs/folder-pane.md).</summary>
    public uint Unread { get; init; }

    /// <summary>The <see cref="Key"/> of the folder this one is filed inside, or <c>null</c> at
    /// the top of the account's tree.</summary>
    /// <remarks>
    /// What the pane nests by. The rows arrive depth-first, each folder ahead of the folders
    /// inside it, so one pass can attach every child to a parent it has already built.
    /// </remarks>
    public string? Parent { get; init; }

    /// <summary>Whether any other folder names this one as its <see cref="Parent"/>.</summary>
    /// <remarks>
    /// NavigationView draws a chevron for an item that has children and none for one that does
    /// not, so this decides the disclosure control by deciding whether the item gets any.
    /// </remarks>
    public bool HasChildren { get; init; }

    /// <summary>Whether the folders inside this one are showing. The core owns and persists it
    /// (docs/folder-pane.md, rule 3).</summary>
    public bool Expanded { get; init; }

    /// <summary>Whether a change to this folder, or to one it sits inside, has not reached the
    /// server yet (docs/folder-pane.md, rule 27). A pending row offers nothing.</summary>
    public bool Pending { get; init; }

    /// <summary>Whether the folder sits inside Trash, where deleting it is permanent.</summary>
    public bool InTrash { get; init; }

    /// <summary>Whether the core offers Rename, Move to… and Delete on this folder, and lets it be
    /// dragged (rule 22).</summary>
    public bool Editable { get; init; }

    /// <summary>Whether a new folder may be made in this one, or a folder dropped onto it.</summary>
    public bool AcceptsFolders { get; init; }

    /// <summary>Whether messages may be dropped onto this folder.</summary>
    public bool AcceptsMessages { get; init; }

    /// <summary>This folder with its tree opened or shut, everything else carried over.</summary>
    public FolderItem WithExpanded(bool expanded) => new()
    {
        Key = Key,
        Name = Name,
        Role = Role,
        Unread = Unread,
        Parent = Parent,
        HasChildren = HasChildren,
        Expanded = expanded,
        Pending = Pending,
        InTrash = InTrash,
        Editable = Editable,
        AcceptsFolders = AcceptsFolders,
        AcceptsMessages = AcceptsMessages,
    };
}

/// <summary>One account in the sidebar switcher: its id, email (display label), and whether its
/// folder tree is open.</summary>
public sealed class AccountItem
{
    /// <summary>The account's id (stable identity, used to select it).</summary>
    public required string Id { get; init; }

    /// <summary>The account's email address (the display label).</summary>
    public required string Email { get; init; }

    /// <summary>How this account reads in a From field: <c>Name &lt;address&gt;</c>, or the
    /// address alone when no name is set. The sidebar shows <see cref="Email"/> instead, which is
    /// what an account is recognised by (<c>docs/sending.md</c>).</summary>
    /// <remarks>
    /// Composed by the core (<c>sender_label</c>) and carried here as data rather than derived on
    /// read: this type compiles into <c>Mailcal.Tests</c>, which links the generated bindings but
    /// loads no cdylib, so a property that called across the FFI would throw in any test that
    /// happened to touch it.
    /// </remarks>
    public required string SendLabel { get; init; }

    /// <summary>Whether this account's folder tree is open, as the core has it persisted.
    /// Independent of which account is selected.</summary>
    public bool Expanded { get; init; }

    /// <summary>Whether this account's folders can be changed at all: its row then offers New
    /// folder and takes a folder dropped on it (docs/folder-pane.md, rule 22).</summary>
    public bool ManagesFolders { get; init; }

    /// <summary>This account's folders, in the core's canonical order. Every account carries its
    /// own, in every view, which is what keeps the other accounts' trees on screen while one of
    /// them is selected.</summary>
    public IReadOnlyList<FolderItem> Folders { get; init; } = [];

    /// <summary>This account with its tree state replaced, everything else carried over.</summary>
    public AccountItem With(bool expanded, IReadOnlyList<FolderItem> folders) => new()
    {
        Id = Id,
        Email = Email,
        SendLabel = SendLabel,
        Expanded = expanded,
        ManagesFolders = ManagesFolders,
        Folders = folders,
    };
}
