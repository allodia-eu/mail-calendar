// The sidebar: a NavigationView accordion, All Inboxes, one expandable entry per account with
// its folders, and Add Account, plus the Calendar footer entry and the Settings gear. The menu is
// BOUND to SidebarItems and reconciled in place from the model's Accounts/Folders/selection, so the
// framework owns container realization and the native selection highlight. Split out of
// MainWindow.xaml.cs to keep that file under the 500-line limit.
//
// It used to be rebuilt by hand, MenuItems.Clear() and a fresh NavigationViewItem tree, once per
// CollectionChanged event, and the model raises one of those per folder. See ViewModels/SidebarTree.cs
// for what that cost and why it is a reconcile now.

using System.Collections.ObjectModel;
using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    // Sentinel tags distinguishing the synthetic sidebar entries from folder keys; an account
    // entry carries AccountTagPrefix + its id, a folder child carries the raw folder key. The
    // three mail-side ones live in SidebarTree, which builds the entries that carry them.
    private const string AllInboxesTag = SidebarTree.AllInboxesTag;
    private const string AddAccountTag = SidebarTree.AddAccountTag;
    private const string AccountTagPrefix = SidebarTree.AccountTagPrefix;
    private const string CalendarTag = "@calendar";
    private const string ContactsTag = "@contacts";

    // Segoe Fluent glyphs: a tray for the unified inbox, a contact for an account, a folder
    // for a mailbox, a plus for add-account.
    //
    // Written as \u escapes, not as the literal characters. These live in the Unicode private use
    // area, where they carry no meaning outside Segoe Fluent Icons and render as nothing (or as a
    // tofu box) in most editors, diffs, and terminals, so a literal is invisible, survives a
    // round-trip through tooling only by luck, and reads as an empty string to anyone reviewing it.
    // Escapes are ASCII, greppable, and say exactly which glyph is meant. (They were briefly lost
    // to precisely that: a refactor retyped them from output that had dropped them, and every
    // sidebar icon silently disappeared.)
    private static readonly SidebarGlyphs Glyphs = new(
        AllInboxes: "\uE715",  // Mail
        Account: "\uE77B",     // Contact
        Folder: "\uE8B7",      // Folder
        AddAccount: "\uE710",  // Add
        ForRole: RoleGlyph);

    /// <summary>
    /// The Segoe Fluent glyph for a folder's special role, a plain folder for anything
    /// without one.
    /// </summary>
    /// <remarks>
    /// Keyed on the role the core resolves (RFC 6154 SPECIAL-USE / JMAP equivalents), never on
    /// the folder's name: the name is whatever the server calls it, so a name test picks the
    /// wrong icon in six of the seven shipped languages, and on any server whose folders were
    /// renamed.
    /// <para>
    /// Every codepoint here was checked against <c>SegoeIcons.ttf</c>'s own cmap and then
    /// rendered and looked at, for the same reason the escapes above exist: a wrong one in this
    /// range is not an error, it is an invisible glyph or a silent tofu box, and the sidebar
    /// keeps drawing as though nothing happened. U+E9D2 sounds like an inbox and draws a bar
    /// chart, which is why Inbox takes the envelope.
    /// </para>
    /// </remarks>
    private static string RoleGlyph(SidebarFolderRole role) => role switch
    {
        SidebarFolderRole.Inbox => "\uE715",   // Mail (an envelope)
        SidebarFolderRole.Drafts => "\uE70F",  // Edit (a pencil)
        SidebarFolderRole.Sent => "\uE724",    // Send (a paper plane)
        SidebarFolderRole.Archive => "\uE7B8", // Archive (a lidded box)
        SidebarFolderRole.Junk => "\uE733",    // Blocked (a barred circle)
        SidebarFolderRole.Trash => "\uE74D",   // Delete (a waste basket)
        // A role we recognise but draw no distinct icon for (flagged / all / important), and
        // every ordinary custom folder, take the plain folder.
        _ => "\uE8B7",                  // Folder
    };

    /// <summary>
    /// The sidebar accordion the NavigationView's <c>MenuItemsSource</c> is bound to. One stable
    /// collection for the window's life, <see cref="SyncNavItems"/> reconciles into it rather than
    /// replacing it, which is what lets the framework keep the containers it has already made.
    /// </summary>
    public ObservableCollection<SidebarItem> SidebarItems { get; } = [];

    /// <summary>
    /// The two non-mail destinations, above the Settings gear, so the accounts/folders accordion
    /// keeps the whole scrollable body to itself. Contacts takes the People glyph, deliberately NOT
    /// the Contact glyph the account rows carry, one card versus a group of them, so the
    /// destination doesn't read as another account.
    /// </summary>
    /// <remarks>
    /// Data rather than the XAML-declared NavigationViewItems these used to be, because a
    /// NavigationView applies its menu-item template to its FOOTER items too: left in XAML they were
    /// re-templated anyway and lost their AutomationId without anything on screen changing. See
    /// <see cref="Views.SidebarItemTemplateSelector"/>. Fixed at construction, unlike the accordion
    /// above, nothing about them varies.
    /// </remarks>
    public ObservableCollection<SidebarItem> FooterItems { get; } =
    [
        new SidebarItem
        {
            Tag = ContactsTag,
            Content = L10n.NavContacts(),
            Glyph = "\uE716",           // People
            AutomationId = "NavContacts",
        },
        new SidebarItem
        {
            Tag = CalendarTag,
            Content = L10n.NavCalendar(),
            Glyph = "\uE787",           // Calendar
            AutomationId = "NavCalendar",
        },
    ];

    /// <summary>
    /// Relabels the NavigationView's built-in Settings item from our own catalog. Its default
    /// label is a WinUI framework resource, resolved against the *OS* language, so it stays
    /// Dutch on a Dutch Windows even when the user (or a screenshot run) has picked English,
    /// which is the one string in the shell our language override can't reach.
    /// </summary>
    private void LocalizeSettingsItem()
    {
        if (Nav.SettingsItem is NavigationViewItem item)
        {
            item.Content = L10n.NavSettings();
        }
    }

    /// <summary>
    /// Brings the accordion in line with the model, then re-asserts the selection highlight.
    /// Cheap and idempotent by design: a signal that changes nothing mutates nothing, so this is
    /// safe to call from every collection/selection/connectivity change without coalescing.
    /// </summary>
    private void SyncNavItems()
    {
        SidebarTree.Reconcile(
            SidebarItems,
            Model.Accounts,
            Model.SelectedAccount,
            // Folders belong on screen only on the mail destination; the calendar and contacts own
            // the highlight outright, and a mail tree behind them is not what the pane is for.
            // Their expansion is untouched, it is the core's, and it is waiting when mail returns.
            showFolders: Model.Destination == AppDestination.Mail,
            Model.UnifiedUnread,
            Model.IsAccountUnreachable,
            OnAccountExpandedChanged,
            new SidebarLabels(
                L10n.SidebarAllInboxes(),
                L10n.ActionAddAccount(),
                // Saturating rather than unchecked: the label is decoration, and a mailbox past
                // int.MaxValue unread should read as "a lot", not wrap to a negative number.
                count => L10n.A11yUnreadCount((int)Math.Min(count, int.MaxValue))),
            Glyphs);
        RestoreSelection();
    }

    /// <summary>
    /// A chevron click: tell the core, which persists it and re-publishes the snapshot.
    /// </summary>
    /// <remarks>
    /// Expanding is not navigating, this deliberately leaves the selected account and folder
    /// alone, which is what lets several accounts stand open at once and what keeps the tree as
    /// the user left it when they visit the calendar (docs/folder-pane.md).
    /// </remarks>
    private void OnAccountExpandedChanged(SidebarItem item)
    {
        if (item.AccountId is { } id)
        {
            Model.SetAccountExpanded(id, item.IsExpanded);
        }
    }

    // Highlights the item matching the model's current scope, so the native selection survives a
    // refresh rather than snapping back to the first item. With a bound menu the selection IS the
    // data item, not its container, the framework maps it to whichever container it has realised.
    private void RestoreSelection()
    {
        // The two footer destinations own the highlight outright; only the mail destination hands
        // it back to an account/folder row.
        if (Model.Destination is AppDestination.Calendar or AppDestination.Contacts)
        {
            var tag = Model.Destination == AppDestination.Calendar ? CalendarTag : ContactsTag;
            Nav.SelectedItem = FooterItems.FirstOrDefault(i => i.Tag == tag);
            return;
        }
        if (Model.SelectedAccount is null)
        {
            Nav.SelectedItem = SidebarItems.FirstOrDefault(i => i.Tag == AllInboxesTag);
            return;
        }
        var account = SidebarItems.FirstOrDefault(i => i.Tag == SidebarTree.TagFor(Model.SelectedAccount));
        if (account is null)
        {
            Nav.SelectedItem = null;
            return;
        }
        // The account row itself is its all-mail view; a child item is one of its folders.
        Nav.SelectedItem = Model.SelectedFolder is null
            ? account
            : account.Children.FirstOrDefault(c => c.Tag == Model.SelectedFolder) ?? account;
    }

    // Opens the unified Settings dialog, then restores the sidebar selection (the Settings gear
    // isn't a scope, so it must not keep the highlight after the modal closes).
    private async Task OpenSettingsAsync(string category = "general")
    {
        var dialog = new SettingsDialog(Model, category) { XamlRoot = Content.XamlRoot };
        _ = await DialogHelper.ShowAsync(dialog);
        RestoreSelection();
    }

    /// <summary>
    /// Right-click an account to remove it (with a confirmation). Raised from the NavigationView
    /// itself and routed to whichever row the pointer was over; a right-click anywhere else in the
    /// pane is left alone.
    /// </summary>
    /// <remarks>
    /// A <c>ContextFlyout</c> declared on the NavigationViewItem inside the item template does not
    /// work, verified, not assumed: with a synthetic right-click that opens the mail list's menu on
    /// the very same run, the account row's produced no popup at all. So the menu is built here,
    /// against the row under the pointer, which also keeps the account id and the label out of a
    /// MenuFlyout's reach, a flyout is not in the visual tree and inherits neither reliably.
    /// </remarks>
    private void OnNavContextRequested(UIElement sender, ContextRequestedEventArgs args)
    {
        if (args.OriginalSource is not DependencyObject source
            || Ancestor<NavigationViewItem>(source) is not { } row
            || row.DataContext is not SidebarItem { AccountId: { } id } item)
        {
            return;
        }
        var remove = new MenuFlyoutItem { Text = L10n.ActionRemoveAccount() };
        var email = item.Content;
        remove.Click += async (_, _) => await ConfirmRemoveAccountAsync(id, email);
        var flyout = new MenuFlyout { Items = { remove } };
        if (args.TryGetPosition(row, out var at))
        {
            flyout.ShowAt(row, new Microsoft.UI.Xaml.Controls.Primitives.FlyoutShowOptions { Position = at });
        }
        else
        {
            flyout.ShowAt(row);
        }
        args.Handled = true;
    }

    // The nearest ancestor of `from` (itself included) of type T, or null.
    private static T? Ancestor<T>(DependencyObject from)
        where T : class
    {
        for (var at = from; at is not null; at = VisualTreeHelper.GetParent(at))
        {
            if (at is T hit)
            {
                return hit;
            }
        }
        return null;
    }

    // Confirms, then removes account `id` (core runtime + stored credential).
    private async Task ConfirmRemoveAccountAsync(string id, string email)
    {
        var result = await DialogHelper.ConfirmAsync(
            Content.XamlRoot,
            L10n.RemoveAccountTitle(),
            L10n.RemoveAccountMessage(email),
            L10n.ActionRemove());
        if (result == ContentDialogResult.Primary)
        {
            Model.RemoveAccount(id);
        }
    }

    private void OnNavItemInvoked(NavigationView sender, NavigationViewItemInvokedEventArgs args)
    {
        // The built-in Settings gear (in the pane footer) opens the unified Settings dialog. It
        // isn't a destination, so restore the scope selection once the dialog closes rather than
        // leaving the gear highlighted.
        if (args.IsSettingsInvoked)
        {
            _ = OpenSettingsAsync();
            return;
        }
        // The tag reaches here the same way whether the row was generated from SidebarItems (the
        // templates bind it) or declared in XAML (the two footer destinations).
        if (args.InvokedItemContainer is not NavigationViewItem item)
        {
            return;
        }
        switch (item.Tag as string)
        {
            case CalendarTag:
                ShowCalendarSurface();
                break;
            case ContactsTag:
                ShowContactsSurface();
                break;
            case AllInboxesTag:
                Model.SelectAccount(null);
                break;
            case AddAccountTag:
                Model.BeginAddAccount();
                break;
            case string tag when tag.StartsWith(AccountTagPrefix, StringComparison.Ordinal):
                Model.SelectAccount(tag[AccountTagPrefix.Length..]);
                break;
            default:
                // A folder child. Its tag is the raw folder key, and a key is unique only within
                // its account, the pane holds every account's tree, so the two travel together
                // into one intent, and the account comes off the row rather than off whatever
                // happens to be selected (docs/folder-pane.md, rule 14).
                if (item.DataContext is SidebarItem { OwnerAccountId: { } owner } row)
                {
                    Model.SelectFolder(owner, row.Tag);
                }
                break;
        }
    }

    /// <summary>
    /// Brings the calendar on screen, opened on today and scrolled to now.
    /// </summary>
    /// <remarks>
    /// The one path to the calendar, shared by the sidebar's Calendar item and the <c>--calendar</c>
    /// launch flag (<see cref="Services.StartupOptions"/>). Both must call
    /// <see cref="Views.CalendarView.OnShown"/>, not just <c>ShowCalendar()</c>: without it the grid
    /// comes up on whatever week it was last left on, at midnight, and on a Sunday (Monday-start
    /// week) today is the last column, six of them off the side of the screen.
    /// </remarks>
    internal void ShowCalendarSurface()
    {
        Model.ShowCalendar();
        CalendarDetail.OnShown();
    }

    /// <summary>
    /// Brings Contacts on screen: the people list beside one person's detail.
    /// </summary>
    /// <remarks>
    /// The one path to Contacts, so the search box and the core's query are cleared together,
    /// <see cref="Views.ContactsView.OnShown"/> empties the field, <see cref="MailboxModel.ShowContacts"/>
    /// the core. Either alone leaves a narrowing the user can no longer see (docs/search.md).
    /// </remarks>
    internal void ShowContactsSurface()
    {
        ContactsDetail.OnShown();
        Model.ShowContacts();
    }
}
