// The fixtures the two sidebar suites share: the labels, the glyphs and the one call into
// SidebarTree. Its own file because the rules and the cost are separate suites (SidebarTreeTests
// and SidebarTreeCostTests), and a fixture copied into both is a fixture that drifts.

using System.Collections.ObjectModel;
using Allodia.Mailcal.ViewModels;

namespace Allodia.Mailcal.Tests;

internal static class SidebarFixture
{
    public static readonly SidebarLabels Labels =
        new("All Accounts", "Inbox", "Add account…", count => $"{count} unread");

    public static readonly SidebarGlyphs Glyphs = new(
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


    public static FolderItem Folder(
        string key, string name, SidebarFolderRole role = SidebarFolderRole.None, uint unread = 0) =>
        new() { Key = key, Name = name, Role = role, Unread = unread };

    /// <summary>The folder list as the model projects one: the synthetic null-key "All Mail" head,
    /// then the real folders.</summary>
    public static List<FolderItem> Folders(int count) =>
        [
            new FolderItem { Key = null, Name = "All Mail" },
            .. Enumerable.Range(0, count).Select(i => Folder($"f{i}", $"Folder {i}")),
        ];

    public static AccountItem Account(
        string id,
        bool expanded = true,
        IReadOnlyList<FolderItem>? folders = null) =>
        new()
        {
            Id = id,
            Email = id + "@example.com",
            // Nameless, which is the ordinary first-run state, so the From label is the address
            // alone. Nothing in these suites draws it; it is here because the record requires it,
            // and a fixture that invented a name would suggest the pane shows one.
            SendLabel = id + "@example.com",
            Expanded = expanded,
            Folders = folders ?? [],
        };

    public static List<AccountItem> Accounts(params string[] ids) =>
        [.. ids.Select(id => Account(id))];

    public static void Sync(
        ObservableCollection<SidebarItem> target,
        IReadOnlyList<AccountItem> accounts,
        string? selected = null,
        bool showFolders = true,
        uint unifiedUnread = 0,
        bool unifiedExpanded = true,
        Func<string, bool>? unreachable = null,
        Action<SidebarItem>? onExpanded = null) =>
        SidebarTree.Reconcile(
            target,
            accounts,
            selected,
            showFolders,
            unifiedUnread,
            unifiedExpanded,
            unreachable ?? (_ => false),
            onExpanded ?? (_ => { }),
            Labels,
            Glyphs);
}
