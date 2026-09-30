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
/// <param name="Outbox">The Outbox row's own label.</param>
/// <param name="UnreadLabel">What a screen reader says for an unread badge, given its count,
/// "545 unread". A delegate rather than a string because it is a formatted message, and this
/// assembly cannot reach L10n.cs.</param>
/// <param name="QueuedLabel">The same for the Outbox's badge, "3 waiting to send". Its own
/// sentence, not <paramref name="UnreadLabel"/>: those are messages nobody has read, these are
/// messages nobody has received, and only one of the two is something to go and do.</param>
/// <param name="Pending">What a folder whose change has not reached the server says it is doing
/// (docs/folder-pane.md, rule 27).</param>
public readonly record struct SidebarLabels(
    string AllAccounts,
    string UnifiedInbox,
    string AddAccount,
    string Outbox,
    Func<uint, string> UnreadLabel,
    Func<uint, string> QueuedLabel,
    string Pending);

/// <summary>The Segoe Fluent glyphs the sidebar entries carry.</summary>
/// <param name="Account">A contact, for an account.</param>
/// <param name="Folder">A folder, for a mailbox with no special role.</param>
/// <param name="AddAccount">A plus, for add-account.</param>
/// <param name="Outbox">An outbound tray, for the Outbox row.</param>
/// <param name="ForRole">The glyph for a folder's special role, supplied by the shell, which is
/// where the codepoints live.</param>
public readonly record struct SidebarGlyphs(
    string Account,
    string Folder,
    string AddAccount,
    string Outbox,
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

    /// <summary>The tag of the Outbox row: every account's unsent messages, in one list.</summary>
    public const string OutboxTag = "@outbox";

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
    /// <param name="queued">How many unsent messages are waiting, across every account. <c>0</c>
    /// draws no Outbox row at all (rule 18).</param>
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
        uint queued,
        Func<string, bool> isUnreachable,
        Action<SidebarItem> onExpandedChanged,
        SidebarLabels labels,
        SidebarGlyphs glyphs,
        Action<Action>? defer = null)
    {
        var wanted = new List<SidebarItem>(accounts.Count + 3);
        var expansion = new List<(SidebarItem Item, bool Expanded)>();

        // The Outbox, above everything, and **only while something is in it** (rule 18). Not
        // inside a tree, because it is not a folder on anybody's server, and not per account,
        // because "did that go?" is not a question about a particular mailbox. Its badge is the
        // one number in this pane that counts something other than unread mail.
        if (queued > 0)
        {
            var outbox = Existing(target, OutboxTag) ?? new SidebarItem
            {
                Tag = OutboxTag,
                Glyph = glyphs.Outbox,
            };
            outbox.Content = labels.Outbox;
            outbox.Unread = queued;
            outbox.UnreadLabel = labels.QueuedLabel(queued);
            wanted.Add(outbox);
        }

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
        ReconcileUnified(group.Children, unifiedUnread, labels, glyphs);
        expansion.Add((group, showFolders && unifiedExpanded));
        wanted.Add(group);

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
            // On an account row this is the account's own capability: New folder, and a folder
            // dropped here to the top of its tree (rule 22).
            item.AcceptsFolders = account.ManagesFolders;
            // Its folders, on every destination, even the ones that draw no mail tree. A row
            // that is emptied and refilled is a row the framework has already realised with
            // nothing in it, and such a row can be told it is open but cannot then show anything
            // (see ApplyExpansion). Shutting it instead is the same thing on screen and leaves
            // the tree it reopens already attached.
            ReconcileFolders(
                item.Children,
                account.Id,
                account.Folders,
                onExpandedChanged,
                labels,
                glyphs,
                expansion);
            expansion.Add((item, showFolders && account.Expanded));
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
        ApplyExpansion(expansion, defer);
    }

    /// <summary>
    /// Mirrors the core's expansion onto every row, once the rows they open are attached.
    /// </summary>
    /// <remarks>
    /// Opening a row is the framework's work rather than a flag we set: a `NavigationViewItem`
    /// realises its children when `IsExpanded` turns true, and it can only realise the children it
    /// has been given. Setting the flag on a row that is already on screen but still empty
    /// therefore moves the chevron and nothing else, and the rows attached a moment later stay
    /// hidden underneath it. Two ordinary things reach that state: a folder found by a sync that
    /// is still running, and leaving mail for the calendar, which empties every account row and
    /// hands it all of its folders back on the way in.
    /// <para>
    /// Attaching the children first is necessary and not sufficient: the item takes them in on its
    /// next layout pass rather than on the collection event. So the host passes
    /// <paramref name="defer"/>, which runs the expansion after that pass, and a caller with no
    /// framework to wait for passes none and it applies inline.
    /// </para>
    /// <para>
    /// `ApplyExpanded`, not the setter: this is the core's own value coming back, and feeding it
    /// to the core again as a "user toggled it" is a rebuild per refresh.
    /// </para>
    /// </remarks>
    private static void ApplyExpansion(
        List<(SidebarItem Item, bool Expanded)> expansion,
        Action<Action>? defer)
    {
        // Only the rows whose state actually moves: the reconcile runs on every refresh, and a
        // deferred no-op per row per refresh is the cost this file exists to avoid.
        var changed = expansion.FindAll(pair => pair.Item.IsExpanded != pair.Expanded);
        if (changed.Count == 0)
        {
            return;
        }
        void Apply()
        {
            foreach (var (item, expanded) in changed)
            {
                item.ApplyExpanded(expanded);
            }
        }
        if (defer is null)
        {
            Apply();
        }
        else
        {
            defer(Apply);
        }
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
        Apply(target, [inbox]);
    }

    /// <summary>
    /// One account's folders, as the tree the provider files them in rather than as a flat list.
    /// </summary>
    /// <remarks>
    /// A folder filed inside another becomes a child of that folder's entry, so the framework
    /// draws the chevron, the indent and the hiding itself: this pane nests rows natively, where
    /// the three that draw a flat list read <c>FolderRow.visible</c> and an indent step instead
    /// (docs/folder-pane.md).
    /// <para>
    /// The rows arrive **depth-first**, each folder ahead of the folders inside it, which is what
    /// lets one pass attach every child to a parent it has already built.
    /// </para>
    /// </remarks>
    private static void ReconcileFolders(
        ObservableCollection<SidebarItem> target,
        string accountId,
        IReadOnlyList<FolderItem> folders,
        Action<SidebarItem> onExpandedChanged,
        SidebarLabels labels,
        SidebarGlyphs glyphs,
        List<(SidebarItem Item, bool Expanded)> expansion)
    {
        var existing = new Dictionary<string, SidebarItem>();
        CollectByTag(target, existing);
        var built = new Dictionary<string, SidebarItem>();
        var children = new Dictionary<string, List<SidebarItem>>();
        var roots = new List<SidebarItem>();

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
            if (!existing.TryGetValue(key, out var item))
            {
                item = new SidebarItem
                {
                    Tag = key,
                    OwnerAccountId = accountId,
                    Glyph = glyphs.ForRole(folder.Role),
                    ExpandedChanged = onExpandedChanged,
                };
            }
            item.Content = folder.Name;
            SetUnread(item, folder.Unread, labels);
            // What the row offers is the core's answer, copied, never decided here (rule 22).
            item.IsPending = folder.Pending;
            item.PendingLabel = folder.Pending ? labels.Pending : string.Empty;
            item.Editable = folder.Editable;
            item.AcceptsFolders = folder.AcceptsFolders;
            item.AcceptsMessages = folder.AcceptsMessages;
            item.InTrash = folder.InTrash;
            // Recorded rather than applied: every row in the pane is opened together once the
            // whole tree is attached, by ApplyExpansion, which says why.
            expansion.Add((item, folder.HasChildren && folder.Expanded));
            built[key] = item;
            // Every folder gets a list, so a folder that has emptied out has its old children
            // removed rather than left on screen under a parent that no longer names them.
            children[key] = [];
            if (folder.Parent is { } parent && built.ContainsKey(parent))
            {
                children[parent].Add(item);
            }
            else
            {
                roots.Add(item);
            }
        }

        foreach (var (key, nested) in children)
        {
            Apply(built[key].Children, nested);
        }
        Apply(target, roots);
    }

    /// <summary>Indexes a whole folder subtree by tag, so a refresh reuses the entries it has.</summary>
    /// <remarks>
    /// The flat <see cref="Existing"/> cannot do this: a folder that was nested last refresh is a
    /// child of its parent's entry, not of the account's, and a lookup that missed it would mint a
    /// second entry and cost the framework a container it had already realised.
    /// </remarks>
    private static void CollectByTag(
        ObservableCollection<SidebarItem> items,
        Dictionary<string, SidebarItem> into)
    {
        foreach (var item in items)
        {
            into[item.Tag] = item;
            CollectByTag(item.Children, into);
        }
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
