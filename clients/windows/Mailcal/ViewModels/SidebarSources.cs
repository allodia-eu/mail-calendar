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
}

/// <summary>One account in the sidebar switcher: its id, email (display label), and whether its
/// folder tree is open.</summary>
public sealed class AccountItem
{
    /// <summary>The account's id (stable identity, used to select it).</summary>
    public required string Id { get; init; }

    /// <summary>The account's email address (the display label).</summary>
    public required string Email { get; init; }

    /// <summary>Whether this account's folder tree is open, as the core has it persisted.
    /// Independent of which account is selected.</summary>
    public bool Expanded { get; init; }

    /// <summary>This account's folders, in the core's canonical order. Every account carries its
    /// own, in every view, which is what keeps the other accounts' trees on screen while one of
    /// them is selected.</summary>
    public IReadOnlyList<FolderItem> Folders { get; init; } = [];
}
