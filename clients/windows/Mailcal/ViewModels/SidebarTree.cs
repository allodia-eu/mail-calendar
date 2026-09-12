// The sidebar's shape, reconciled IN PLACE against the model's accounts/folders/selection.
//
// Why in place, and why this file exists at all: the sidebar used to be rebuilt from scratch,
// NavigationView.MenuItems.Clear() and a fresh NavigationViewItem tree, on every CollectionChanged
// event. The model refills its Folders collection with Clear() + one Add() per folder, so selecting
// an account with N folders meant N+2 complete rebuilds, each itself O(N). Measured on a real
// 57-folder account that was 60 rebuilds and TEN SECONDS of frozen UI thread (sixteen on the second
// visit, because the discarded trees were retained and every later rebuild got dearer). The core's
// half of the same refresh takes 2 ms.
//
// So the shape is computed here and the framework is told only what changed. The same N+2 signals
// now cost N removes and N adds of DATA, all inside one dispatcher turn, so XAML lays out once.
//
// WinUI-free on purpose, linked into Mailcal.Tests (see SidebarItem). The strings arrive as
// parameters because L10n.cs cannot be linked into a plain net10.0 assembly.

using System.Collections.ObjectModel;

namespace Allodia.Mailcal.ViewModels;

/// <summary>The localised labels and strings the synthetic sidebar entries carry.</summary>
/// <param name="AllAccounts">The All Accounts group's heading.</param>
/// <param name="UnifiedInbox">The group's one child, the unified Inbox. The Inbox role's own name
/// from the app's catalog, the same word the message-list header uses for that scope
/// (docs/folder-pane.md, rule 13).</param>
/// <param name="AddAccount">The "add another account" action.</param>
/// <param name="UnreadLabel">What a screen reader says for an unread badge, given its count,
/// "545 unread". A delegate rather than a string because it is a formatted message, and this
/// assembly cannot reach L10n.cs.</param>
public readonly record struct SidebarLabels(
    string AllAccounts,
    string UnifiedInbox,
    string AddAccount,
    Func<uint, string> UnreadLabel);

/// <summary>The Segoe Fluent glyphs the sidebar entries carry.</summary>
/// <param name="Account">A contact, for an account.</param>
/// <param name="Folder">A folder, for a mailbox with no special role.</param>
/// <param name="AddAccount">A plus, for add-account.</param>
/// <param name="ForRole">The glyph for a folder's special role, supplied by the shell, which is
/// where the codepoints live.</param>
public readonly record struct SidebarGlyphs(
    string Account,
    string Folder,
    string AddAccount,
    Func<SidebarFolderRole, string> ForRole);

/// <summary>Reconciles the bound sidebar collection toward the model's current state.</summary>
public static class SidebarTree
{
    /// <summary>The tag of the All Accounts group's heading row.</summary>
    public const string AllAccountsTag = "@all-accounts";

    /// <summary>The tag of the unified Inbox, the All Accounts group's one child.</summary>
    public const string AllInboxesTag = "@all-inboxes";

    /// <summary>The tag of the "add another account" action.</summary>
    public const string AddAccountTag = "@add-account";

    /// <summary>The prefix an account entry's tag carries, ahead of the account id.</summary>
    public const string AccountTagPrefix = "acct:";

    /// <summary>The tag an account entry carries.</summary>
    public static string TagFor(string accountId) => AccountTagPrefix + accountId;

    /// <summary>
    /// Brings <paramref name="target"/> in line with the model: the All Accounts group over the
    /// unified Inbox, one entry per account over its own folders, then Add account. Entries that
    /// are already right are left alone, including their generated containers, so a refresh that
    /// changes nothing mutates nothing.
    /// </summary>
    /// <remarks>
    /// Three rules this shape exists to hold (docs/folder-pane.md). **Every** account carries its
    /// folders, not just the selected one, the pane used to be fed the selected account's folders
    /// alone, so choosing the unified Inbox emptied it. Expansion comes from
    /// <see cref="AccountItem.Expanded"/> and <paramref name="unifiedExpanded"/>, which the core
    /// persists, rather than from <paramref name="selectedAccount"/>, so selecting an account no
    /// longer shuts the account beside it, and the trees survive a restart. And the unified list is
    /// a **group**, not a row (rule 16): the heading navigates nowhere, and the Inbox under it is
    /// what carries the badge and the destination.
    /// </remarks>
    /// <param name="target">The collection the NavigationView is bound to.</param>
    /// <param name="accounts">The configured accounts, in the core's order, each with its folders.</param>
    /// <param name="selectedAccount">The selected account's id, or <c>null</c> for the unified view.</param>
    /// <param name="showFolders">Whether folders belong on screen at all, false on the calendar and
    /// contacts destinations, where the mail tree is not what the pane is for.</param>
    /// <param name="unifiedUnread">The unified Inbox badge: every account's Inbox unread, summed.</param>
    /// <param name="unifiedExpanded">Whether the All Accounts group's tree is open, the core's own
    /// value (<c>MailboxListSnapshot.UnifiedExpanded</c>).</param>
    /// <param name="isUnreachable">Whether an account's server couldn't be reached on its last sync.</param>
    /// <param name="onExpandedChanged">Where a user's chevron click goes, the shell dispatches it
    /// to the core, which persists it.</param>
    /// <param name="labels">The localised synthetic labels.</param>
    /// <param name="glyphs">The Segoe Fluent glyphs.</param>
    public static void Reconcile(
        ObservableCollection<SidebarItem> target,
        IReadOnlyList<AccountItem> accounts,
        string? selectedAccount,
        bool showFolders,
        uint unifiedUnread,
        bool unifiedExpanded,
        Func<string, bool> isUnreachable,
        Action<SidebarItem> onExpandedChanged,
        SidebarLabels labels,
        SidebarGlyphs glyphs)
    {
        // The group's heading. No glyph and no destination: its row IS the disclosure control
        // (rule 17), and an icon beside the accounts' would read as one more account row, which is
        // the thing that rule exists to prevent. The chevron the framework draws for an item with
        // children is what says which way activating it goes.
        var group = Existing(target, AllAccountsTag) ?? new SidebarItem
        {
            Tag = AllAccountsTag,
            IsGroup = true,
            SelectsOnInvoked = false,
            ExpandedChanged = onExpandedChanged,
        };
        group.Content = labels.AllAccounts;
        // `ApplyExpanded`, not the setter: this is the core's own value coming back, and feeding
        // it to the core again as a "user toggled it" is a rebuild per refresh.
        group.ApplyExpanded(showFolders && unifiedExpanded);
        ReconcileUnified(group.Children, showFolders, unifiedUnread, labels, glyphs);
        var wanted = new List<SidebarItem>(accounts.Count + 2) { group };

        foreach (var account in accounts)
        {
            var tag = TagFor(account.Id);
            var item = Existing(target, tag) ?? new SidebarItem
            {
                Tag = tag,
                AccountId = account.Id,
                Glyph = glyphs.Account,
                ExpandedChanged = onExpandedChanged,
            };
            item.Content = account.Email;
            item.ShowBadge = isUnreachable(account.Id);
            // `ApplyExpanded`, not the setter: this is the core's own value coming back, and
            // feeding it to the core again as a "user toggled it" is a rebuild per refresh.
            item.ApplyExpanded(showFolders && account.Expanded);
            ReconcileFolders(
                item.Children, account.Id, showFolders ? account.Folders : [], labels, glyphs);
            wanted.Add(item);
        }

        var add = Existing(target, AddAccountTag) ?? new SidebarItem
        {
            Tag = AddAccountTag,
            Glyph = glyphs.AddAccount,
            // An action, not a destination, so it never holds the selection.
            SelectsOnInvoked = false,
        };
        add.Content = labels.AddAccount;
        wanted.Add(add);

        Apply(target, wanted);
    }

    /// <summary>
    /// The All Accounts group's children: the unified Inbox, and nothing else yet.
    /// </summary>
    /// <remarks>
    /// A unified Sent, Drafts and Archive are the shape the group exists for, and none of them
    /// exists: the core's unified scope reaches every account's Inbox only, so a second child would
    /// need a scope to select before it needed a row to sit on (docs/folder-pane.md).
    /// <para>
    /// The badge is here rather than on the heading, for the reason an account row carries none: a
    /// roll-up would sit directly above the identical number on the row beneath it (rule 8).
    /// </para>
    /// </remarks>
    private static void ReconcileUnified(
        ObservableCollection<SidebarItem> target,
        bool showFolders,
        uint unifiedUnread,
        SidebarLabels labels,
        SidebarGlyphs glyphs)
    {
        var inbox = Existing(target, AllInboxesTag) ?? new SidebarItem
        {
            Tag = AllInboxesTag,
            Glyph = glyphs.ForRole(SidebarFolderRole.Inbox),
        };
        inbox.Content = labels.UnifiedInbox;
        SetUnread(inbox, unifiedUnread, labels);
        List<SidebarItem> wanted = showFolders ? [inbox] : [];
        Apply(target, wanted);
    }

    private static void ReconcileFolders(
        ObservableCollection<SidebarItem> target,
        string accountId,
        IReadOnlyList<FolderItem> folders,
        SidebarLabels labels,
        SidebarGlyphs glyphs)
    {
        var wanted = new List<SidebarItem>(folders.Count);
        foreach (var folder in folders)
        {
            // The synthetic null-key "All Mail" head is skipped: the account row itself is that view.
            if (folder.Key is not { } key)
            {
                continue;
            }
            // Each folder carries the account it belongs to, because its key does not name a
            // mailbox on its own: the two travel together in one intent (docs/folder-pane.md,
            // rule 14).
            var item = Existing(target, key)
                ?? new SidebarItem
                {
                    Tag = key,
                    OwnerAccountId = accountId,
                    Glyph = glyphs.ForRole(folder.Role),
                };
            item.Content = folder.Name;
            SetUnread(item, folder.Unread, labels);
            wanted.Add(item);
        }
        Apply(target, wanted);
    }

    /// <summary>Sets a row's unread count and the sentence a screen reader reads for it.</summary>
    /// <remarks>
    /// The label is only built when there is a badge to describe: at zero the badge is not drawn,
    /// and formatting a string for an invisible element on every row of every refresh is work the
    /// reconcile exists to avoid.
    /// </remarks>
    private static void SetUnread(SidebarItem item, uint unread, SidebarLabels labels)
    {
        item.Unread = unread;
        item.UnreadLabel = unread > 0 ? labels.UnreadLabel(unread) : string.Empty;
    }

    private static SidebarItem? Existing(ObservableCollection<SidebarItem> target, string tag)
    {
        foreach (var item in target)
        {
            if (item.Tag == tag)
            {
                return item;
            }
        }
        return null;
    }

    /// <summary>
    /// Moves <paramref name="target"/> onto <paramref name="wanted"/> with the fewest collection
    /// events: entries already in the right place raise none at all, which is the whole point,
    /// each one the framework hears about is a container it has to realise.
    /// </summary>
    private static void Apply(ObservableCollection<SidebarItem> target, List<SidebarItem> wanted)
    {
        var keep = new HashSet<string>(wanted.Count);
        foreach (var item in wanted)
        {
            keep.Add(item.Tag);
        }
        for (var i = target.Count - 1; i >= 0; i--)
        {
            if (!keep.Contains(target[i].Tag))
            {
                target.RemoveAt(i);
            }
        }
        // Earlier positions are already final, so the entry for position i is at i or ahead of it.
        for (var i = 0; i < wanted.Count; i++)
        {
            if (i < target.Count && ReferenceEquals(target[i], wanted[i]))
            {
                continue;
            }
            var found = -1;
            for (var j = i + 1; j < target.Count; j++)
            {
                if (ReferenceEquals(target[j], wanted[i]))
                {
                    found = j;
                    break;
                }
            }
            if (found >= 0)
            {
                target.Move(found, i);
            }
            else
            {
                target.Insert(i, wanted[i]);
            }
        }
        while (target.Count > wanted.Count)
        {
            target.RemoveAt(target.Count - 1);
        }
    }
}
