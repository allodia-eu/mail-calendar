// The bindable view helpers: what the XAML reads so it needs no converters. Split out of
// MailboxModel.cs to keep that file under the 500-line limit.
//
// Almost everything here is derived rather than stored: each one is a projection of the reactive
// state next door, so the one place a value can be wrong is the state it is read from, and the
// `Raise` calls that announce them stay with that state. The exception is the search query, which
// is view state of its own and sits beside the title it decides.

using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>Whether the send hint should show (a send is in flight or just finished).</summary>
    /// <remarks>SentNotFiled shows no transient hint: the standing UnfiledCopy question already says this, and says it with a button.</remarks>
    public bool SendStatusVisible =>
        _sendStatus != SendStatus.Idle && _sendStatus != SendStatus.SentNotFiled;
    /// <summary>Shows the in-flight spinner only while the send hasn't completed yet.</summary>
    public Visibility SendStatusBusyVisibility =>
        _sendStatus == SendStatus.Sending ? Visibility.Visible : Visibility.Collapsed;
    /// <summary>The send hint text for the current status.</summary>
    public string SendStatusText => _sendStatus switch
    {
        SendStatus.Sending => L10n.SendStatusSending(),
        SendStatus.Sent => L10n.SendStatusSent(),
        SendStatus.Failed => L10n.SendStatusFailed(),
        _ => string.Empty,
    };
    /// <summary>The info-bar severity for the current status.</summary>
    public Microsoft.UI.Xaml.Controls.InfoBarSeverity SendStatusSeverity => _sendStatus switch
    {
        SendStatus.Failed => Microsoft.UI.Xaml.Controls.InfoBarSeverity.Error,
        SendStatus.Sent => Microsoft.UI.Xaml.Controls.InfoBarSeverity.Success,
        _ => Microsoft.UI.Xaml.Controls.InfoBarSeverity.Informational,
    };

    // Background sync progress, the awaited download's bar and the background hint beside the
    // footer's connection status, lives in MailboxModel.SyncProgress.cs.

    // Connectivity, the offline state, the per-account unreachable outages, and the OS network
    // watch that feeds the core, lives in MailboxModel.Connectivity.cs.

    // The three top-level surfaces are mutually exclusive, and the welcome screen outranks both of
    // the others: it is the first thing a new user sees, ahead of setup. `AnalyticsAsked` (see
    // MailboxModel.Analytics.cs) reads true before the core has connected, so nothing flashes while
    // the app is still starting.

    /// <summary>Show the setup form on first run, or while adding another account.</summary>
    public Visibility SetupVisibility =>
        AnalyticsAsked && (NeedsSetup || AddingAccount) ? Visibility.Visible : Visibility.Collapsed;
    /// <summary>Show the main shell once connected (hidden behind the welcome/setup screens).</summary>
    public Visibility MainVisibility =>
        AnalyticsAsked && !NeedsSetup && !AddingAccount ? Visibility.Visible : Visibility.Collapsed;
    /// <summary>Show the setup form's Cancel button only when adding another account (not first run).</summary>
    public Visibility AddingAccountVisibility => AddingAccount ? Visibility.Visible : Visibility.Collapsed;
    /// <summary>
    /// Show the form's Cancel button when adding another account OR whenever a Microsoft sign-in is
    /// in flight, the browser step can hang indefinitely (the user closes the tab, or picks the
    /// wrong app on the redirect), so a first-run sign-in needs an escape too, not only an add.
    /// </summary>
    public Visibility CancelVisibility =>
        AddingAccount || IsSigningIn ? Visibility.Visible : Visibility.Collapsed;
    /// <summary>
    /// The Cancel button is enabled while a Microsoft sign-in is in flight (so it can abort the
    /// hung wait), and otherwise whenever nothing is submitting. A bounded IMAP/JMAP connect still
    /// disables it, that call errors out on its own, but the unbounded browser wait must not.
    /// </summary>
    public bool CancelEnabled => IsSigningIn || NotSubmitting;
    /// <summary>Whether nothing is in flight, gates the form's Cancel button.</summary>
    public bool NotSubmitting => !IsSubmitting;
    /// <summary>Show the setup form's connect spinner only while a connect/add is in flight.</summary>
    public Visibility SubmittingVisibility => IsSubmitting ? Visibility.Visible : Visibility.Collapsed;
    /// <summary>The Connect button's label: a "connecting" status while in flight, else "Connect".</summary>
    public string ConnectButtonText => IsSubmitting ? L10n.StatusConnecting() : L10n.ActionConnect();
    /// <summary>Show the mailbox detail when the mail destination is active.</summary>
    public Visibility MailVisibility =>
        Destination == AppDestination.Mail ? Visibility.Visible : Visibility.Collapsed;
    /// <summary>Show the calendar detail when active.</summary>
    public Visibility CalendarVisibility =>
        Destination == AppDestination.Calendar ? Visibility.Visible : Visibility.Collapsed;
    /// <summary>Show the contacts detail when active.</summary>
    public Visibility ContactsVisibility =>
        Destination == AppDestination.Contacts ? Visibility.Visible : Visibility.Collapsed;
    /// <summary>Whether a setup error should be surfaced.</summary>
    public bool HasSetupError => !string.IsNullOrEmpty(SetupError);
    /// <summary>Whether the device-zone-changed prompt should show.</summary>
    public bool HasPendingZone => PendingDeviceZone is not null;
    /// <summary>The body text of the zone-changed prompt.</summary>
    public string ZonePromptText =>
        L10n.TzChangedMessage(PendingDeviceZone ?? L10n.TzZoneNew());
    /// <summary>The "keep current" button label of the zone-changed prompt.</summary>
    public string KeepZoneText => L10n.TzKeep(ActiveZone);

    /// <summary>
    /// The current scope's title: the unified Inbox (no account selected), else the selected
    /// folder's name or the account's "All Mail".
    /// </summary>
    /// <remarks>
    /// The unified scope is named by the row that opens it, not by the group above it: the header
    /// reads "Inbox" because "All Accounts" is a word no row the user can select carries
    /// (docs/folder-pane.md, rule 13).
    /// </remarks>
    public string CurrentFolderName
    {
        get
        {
            if (SelectedAccount is null)
            {
                return FolderLabel.Unified();
            }
            if (SelectedFolder is null)
            {
                return L10n.SidebarAllMail();
            }
            foreach (var folder in Folders)
            {
                if (folder.Key == SelectedFolder)
                {
                    return folder.Name;
                }
            }
            return L10n.FolderFallback();
        }
    }

    private string _searchQuery = string.Empty;

    /// <summary>
    /// What stands in the search field right now, set by the shell as the user types.
    /// </summary>
    /// <remarks>
    /// Held here rather than read back off the field because the header it re-labels lives in
    /// another view: the field is the window's (docs/search.md), the header is the message list's.
    /// It is deliberately **not** the query the core has been asked, which the debounce holds back
    /// by a quarter of a second: the header describes the field, not the results.
    /// </remarks>
    public string SearchQuery
    {
        private get => _searchQuery;
        set { if (Set(ref _searchQuery, value)) { Raise(nameof(ListTitle)); } }
    }

    /// <summary>What the message list calls itself: the current scope, or "Search results" while
    /// the search field has anything in it.</summary>
    public string ListTitle =>
        string.IsNullOrEmpty(_searchQuery) ? CurrentFolderName : L10n.SearchResults();

    /// <summary>The mailbox footer count ("N messages" / "N conversations"), the folder's
    /// full total, not the visible window (<see cref="Rows"/> holds only the loaded page).</summary>
    public string MailCountText =>
        Mode == ViewModeKind.Threaded
            ? L10n.MailboxCountConversations((int)_total)
            : L10n.MailboxCountMessages((int)_total);
}
