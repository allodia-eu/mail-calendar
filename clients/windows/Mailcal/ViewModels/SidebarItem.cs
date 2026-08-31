// One node of the sidebar accordion, All Inboxes, an account (with its folders as children), a
// folder, or Add account. The NavigationView binds to a collection of these rather than being
// rebuilt by hand, so the framework realises and recycles containers itself and an unchanged node
// keeps the one it has.
//
// WinUI-free on purpose, like the calendar's pure layer: it is linked into Mailcal.Tests, which is
// a plain net10.0 assembly, so the tree-shaping rules in SidebarTree are pinned by tests that need
// no window. That is why the unreachable badge is a bool here and becomes a Visibility only in the
// DataTemplate (SidebarTemplates.cs), and why the display strings arrive as parameters, L10n.cs
// cannot be linked into that assembly.

using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace Allodia.Mailcal.ViewModels;

/// <summary>One entry in the sidebar accordion, with its folder children when it is an account.</summary>
/// <remarks>
/// The four identity properties below would rather be <c>required init</c>, and cannot be: a type
/// named in an <c>x:DataType</c> gets an entry in the generated XamlTypeInfo, which activates it
/// parameterlessly and assigns through plain setters. So they are settable by the compiler's
/// reckoning and write-once by ours, <see cref="SidebarTree"/> sets them when it mints an entry
/// and never again, because they are the identity it matches on.
/// </remarks>
public sealed class SidebarItem : INotifyPropertyChanged
{
    /// <summary>
    /// What this entry selects: a sentinel (<c>@all-inboxes</c>, <c>@add-account</c>), an account
    /// (<c>acct:&lt;id&gt;</c>), or a folder's raw provider key. It rides onto the generated
    /// container's <c>Tag</c>, which is what the shell's ItemInvoked handler reads, the same
    /// contract the hand-built items carried, so the two agree.
    /// </summary>
    public string Tag { get; set; } = string.Empty;

    /// <summary>The account's id on an account entry; <c>null</c> on every other kind. Also the
    /// discriminator the template selector uses, since only an account row offers "Remove account"
    /// and only an account row has children.</summary>
    public string? AccountId { get; set; }

    /// <summary>
    /// On a folder entry, the id of the account whose tree it hangs in; <c>null</c> on every other
    /// kind, including an account entry, which carries its own id in <see cref="AccountId"/>.
    /// </summary>
    /// <remarks>
    /// A folder's <see cref="Tag"/> is its raw provider key, and a key is unique only WITHIN an
    /// account, every provider calls its inbox <c>inbox</c>. The pane holds every account's tree at
    /// once, so the key alone does not say which mailbox a row points at (docs/folder-pane.md).
    /// Kept separate from <see cref="AccountId"/> because that one is the template discriminator:
    /// setting it here would give every folder the account row's "Remove account" menu.
    /// </remarks>
    public string? OwnerAccountId { get; set; }

    /// <summary>The Segoe Fluent glyph for this entry.</summary>
    public string Glyph { get; set; } = string.Empty;

    /// <summary>
    /// The entry's UI-Automation id, for the entries a test needs to address by name rather than by
    /// label, the two footer destinations, whose labels are localised. <c>null</c> everywhere else,
    /// where the label is the identity.
    /// </summary>
    public string? AutomationId { get; set; }

    /// <summary>Whether invoking this entry holds the selection. False for Add account, which is
    /// an action rather than a destination.</summary>
    public bool SelectsOnInvoked { get; set; } = true;

    /// <summary>This account's folders. Empty on every other kind. Populated for **every**
    /// account, not just the selected one, the pane shows every tree at once, and each account's
    /// own <see cref="IsExpanded"/> decides whether its folders are on screen.</summary>
    public ObservableCollection<SidebarItem> Children { get; } = [];

    /// <summary>
    /// Called when the **user** opens or shuts this account's tree, so the change reaches the core
    /// and is persisted. Not raised while <see cref="ApplyExpanded"/> is writing the core's own
    /// value back in, which would otherwise echo every refresh back as a fresh user action.
    /// </summary>
    public Action<SidebarItem>? ExpandedChanged { get; set; }

    private string _content = string.Empty;

    /// <summary>The label the user reads (an account's email, a folder's name).</summary>
    public string Content
    {
        get => _content;
        set => Set(ref _content, value);
    }

    private bool _isExpanded;
    private bool _applyingCoreState;

    /// <summary>
    /// Whether this account's folders are showing. Two-way bound, so the chevron the user clicks
    /// writes back here, and from here, through <see cref="ExpandedChanged"/>, to the core, which
    /// persists it. The core is the owner: this is a mirror of `AccountRow.expanded`, never the
    /// authority (docs/folder-pane.md).
    /// </summary>
    public bool IsExpanded
    {
        get => _isExpanded;
        set
        {
            if (Set(ref _isExpanded, value) && !_applyingCoreState)
            {
                ExpandedChanged?.Invoke(this);
            }
        }
    }

    /// <summary>
    /// Writes the core's expansion state in without treating it as a user action.
    /// </summary>
    /// <remarks>
    /// Without this, every reconcile would feed the value the core just gave us straight back to
    /// the core as a fresh toggle. The core happens to no-op an unchanged value, so the bug would
    /// not be visible, it would just mean a dispatch, a snapshot rebuild and a full sidebar
    /// reconcile per account per refresh, which is the exact cost this whole file exists to avoid.
    /// </remarks>
    public void ApplyExpanded(bool expanded)
    {
        _applyingCoreState = true;
        try
        {
            IsExpanded = expanded;
        }
        finally
        {
            _applyingCoreState = false;
        }
    }

    private uint _unread;

    /// <summary>How many messages this folder holds unread, as the server counts them.</summary>
    public uint Unread
    {
        get => _unread;
        set
        {
            if (Set(ref _unread, value))
            {
                OnPropertyChanged(nameof(UnreadText));
                OnPropertyChanged(nameof(ShowUnread));
            }
        }
    }

    /// <summary>The count as the row draws it, in the user's own digits.</summary>
    public string UnreadText => _unread.ToString(System.Globalization.CultureInfo.CurrentCulture);

    /// <summary>Whether the badge is drawn at all, never at zero (docs/folder-pane.md).</summary>
    public bool ShowUnread => _unread > 0;

    private string _unreadLabel = string.Empty;

    /// <summary>
    /// What a screen reader says for the badge ("545 unread"), localised by the caller.
    /// </summary>
    /// <remarks>
    /// The bare number is meaningless read aloud: "Inbox, 545" could be a position in a list. Set
    /// from L10n by the reconcile, because this file cannot reach L10n.cs (it is linked into a
    /// plain net10.0 assembly).
    /// </remarks>
    public string UnreadLabel
    {
        get => _unreadLabel;
        set => Set(ref _unreadLabel, value);
    }

    private bool _showBadge;

    /// <summary>Whether this account's server couldn't be reached on its last sync while online,
    /// drives the warning badge, distinct from the device-wide offline banner.</summary>
    public bool ShowBadge
    {
        get => _showBadge;
        set => Set(ref _showBadge, value);
    }

    /// <inheritdoc/>
    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>Assigns <paramref name="field"/>, notifying only on a real change. Returns whether
    /// the value moved, so a setter can chain dependent notifications (or a callback) off it.</summary>
    private bool Set<T>(ref T field, T value, [CallerMemberName] string? name = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value))
        {
            return false;
        }
        field = value;
        OnPropertyChanged(name);
        return true;
    }

    private void OnPropertyChanged(string? name) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
