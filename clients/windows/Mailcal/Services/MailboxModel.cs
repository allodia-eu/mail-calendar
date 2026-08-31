// The WinUI source of truth and the Rust-driven Observer bridge, the Windows counterpart
// of macOS's MailcalModel.swift. SurfaceObserver hands Rust's surface-changed callback to
// the UI DispatcherQueue; the model dispatches intents into the Rust app and pulls
// immutable snapshots, projecting them into the public ViewModel collections the XAML
// binds to. State stays in Rust; diffing/rendering stays in WinUI, with one clean thread
// hop. The generated UniFFI types (internal) are confined to this service layer.

using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Threading.Tasks;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Win32;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>
/// Bridges the Rust-driven <c>Observer</c> callback (invoked from a runtime thread) into a
/// delegate; the model marshals it onto the UI thread.
/// </summary>
internal sealed class SurfaceObserver : Observer
{
    private readonly Action<Surface> _onChange;
    public SurfaceObserver(Action<Surface> onChange) => _onChange = onChange;
    public void SurfaceChanged(Surface @surface) => _onChange(@surface);
}

/// <summary>How the mailbox list is grouped (the public mirror of the FFI ViewMode).</summary>
public enum ViewModeKind
{
    /// <summary>A flat list of messages, newest first.</summary>
    Flat,

    /// <summary>Conversations grouped by thread.</summary>
    Threaded,
}

/// <summary>The WinUI source of truth, driven by the Rust app.</summary>
public sealed partial class MailboxModel : INotifyPropertyChanged
{
    private readonly DispatcherQueue _ui = DispatcherQueue.GetForCurrentThread();
    private MailcalApp? _app;
    private SurfaceObserver? _observer;
    private CoreLogger? _logger;
    private bool _watchingTimeZone;
    private bool _connecting;

    /// <summary>The configured accounts, for the sidebar switcher.</summary>
    public ObservableCollection<AccountItem> Accounts { get; } = new();

    /// <summary>The selected account's folders (with a synthetic "All Mail" head).</summary>
    /// <remarks>
    /// **Not the folder pane's source**, that is <see cref="Accounts"/>, where each account
    /// carries its own tree in every view. This one still backs the surfaces that are genuinely
    /// about the selected account alone (docs/folder-pane.md).
    /// </remarks>
    public ObservableCollection<FolderItem> Folders { get; } = new();

    /// <summary>The All Inboxes badge: every account's Inbox unread, summed. <c>0</c> shows none.</summary>
    public uint UnifiedUnread { get; private set; }

    /// <summary>The mailbox rows currently shown, the visible window (the first page, grown
    /// by <see cref="ShowMore"/> as the list scrolls), in display order.</summary>
    public ObservableCollection<MailRow> Rows { get; } = new();

    // Pagination: the full row count for the current view (set from each snapshot) and a guard
    // that coalesces the burst of scroll events into one in-flight "show more" request.
    private ulong _total;
    private bool _loadMorePending;

    /// <summary>Whether more rows can be shown than are currently in <see cref="Rows"/>, the
    /// view checks this as it scrolls toward the end before asking for the next page.</summary>
    public bool HasMore => (ulong)Rows.Count < _total;

    /// <summary>The calendar agenda rows, soonest first.</summary>
    public ObservableCollection<EventItem> Events { get; } = new();

    /// <summary>
    /// Every IANA zone the engine can localise against, for the time-zone picker, one
    /// authoritative list sourced from the engine's bundled tzdb (shared across clients),
    /// not the host OS's zone set, which on Windows collapses cities like Europe/Amsterdam.
    /// </summary>
    public IReadOnlyList<string> AvailableZones { get; } = MailcalBindingsMethods.AvailableTimeZones();

    /// <summary>The host's writable app-data dir (engine store + zone preference live here).</summary>
    private static string DataDir => AppPaths.Root;

    // --- Reactive scalar state ------------------------------------------------

    private bool _needsSetup;
    /// <summary><c>true</c> when no account is configured yet, show the full-screen setup form.</summary>
    public bool NeedsSetup
    {
        get => _needsSetup;
        private set { if (Set(ref _needsSetup, value)) { Raise(nameof(SetupVisibility)); Raise(nameof(MainVisibility)); } }
    }

    private bool _addingAccount;
    /// <summary>
    /// <c>true</c> while the user is adding another account: the setup form shows over the
    /// running app (the same form as first run, with a Cancel) and the shell hides behind it.
    /// </summary>
    public bool AddingAccount
    {
        get => _addingAccount;
        private set
        {
            if (Set(ref _addingAccount, value))
            {
                Raise(nameof(SetupVisibility));
                Raise(nameof(MainVisibility));
                Raise(nameof(AddingAccountVisibility));
                Raise(nameof(CancelVisibility));
            }
        }
    }

    private bool _submitting;
    /// <summary>
    /// <c>true</c> while a connect/add-account is in flight (the network login blocks on a
    /// background thread), drives the setup form's loading state: the Connect button shows a
    /// spinner and a "connecting" label and is disabled, so the user can't submit twice.
    /// </summary>
    public bool IsSubmitting
    {
        get => _submitting;
        private set
        {
            if (Set(ref _submitting, value))
            {
                Raise(nameof(NotSubmitting));
                Raise(nameof(SubmittingVisibility));
                Raise(nameof(ConnectButtonText));
                Raise(nameof(CancelEnabled));
            }
        }
    }

    // The single browser sign-in in flight, shared by the Microsoft and Google flows: only one may
    // be outstanding (they compete for one redirect rendezvous), and a fresh request supersedes an
    // attempt the user abandoned by closing the browser tab, which is undetectable, so refusing
    // the second request left the reconnect banner's button dead. See SignInFlight.
    private readonly SignInFlight _signIn = new();

    private bool _signingIn;
    /// <summary>
    /// <c>true</c> while a browser sign-in is out in the user's browser. Unlike a bounded
    /// IMAP/JMAP connect, this can hang indefinitely if the browser step is abandoned, so the form
    /// surfaces a Cancel that calls <see cref="CancelMicrosoftSignIn"/> (see <see cref="CancelVisibility"/>).
    /// </summary>
    public bool IsSigningIn
    {
        get => _signingIn;
        private set
        {
            if (Set(ref _signingIn, value))
            {
                Raise(nameof(CancelVisibility));
                Raise(nameof(CancelEnabled));
            }
        }
    }

    private string? _selectedAccount;
    /// <summary>The selected account's id, or <c>null</c> for the unified "all inboxes" view.</summary>
    public string? SelectedAccount
    {
        get => _selectedAccount;
        // The footer connection-status label is scoped to the selected account (or all accounts in
        // the unified view), so refresh it when the scope changes.
        private set { if (Set(ref _selectedAccount, value)) { Raise(nameof(CurrentFolderName)); RaiseConnectionStatus(); } }
    }

    private string? _setupError;
    /// <summary>A setup/connect error to surface on the form, or <c>null</c>.</summary>
    public string? SetupError
    {
        get => _setupError;
        private set { if (Set(ref _setupError, value)) { Raise(nameof(HasSetupError)); } }
    }

    private AppDestination _destination = AppDestination.Mail;
    /// <summary>
    /// Which top-level surface is on screen. An enum rather than a flag per screen: with three
    /// destinations, booleans admit states that cannot exist ("the calendar and contacts at once")
    /// and every reader has to prove they don't happen.
    /// </summary>
    public AppDestination Destination
    {
        get => _destination;
        private set
        {
            if (Set(ref _destination, value))
            {
                Raise(nameof(MailVisibility));
                Raise(nameof(CalendarVisibility));
                Raise(nameof(ContactsVisibility));
            }
        }
    }

    private ViewModeKind _mode = ViewModeKind.Flat;
    /// <summary>The mode the rows are grouped in.</summary>
    public ViewModeKind Mode
    {
        get => _mode;
        private set { if (Set(ref _mode, value)) { Raise(nameof(IsThreaded)); Raise(nameof(MailCountText)); } }
    }

    /// <summary>Whether the list is grouped into threads (drives the header toggle).</summary>
    public bool IsThreaded => Mode == ViewModeKind.Threaded;

    private string? _selectedFolder;
    /// <summary>The selected folder's key, or <c>null</c> for the selected account's all-mail view.</summary>
    public string? SelectedFolder
    {
        get => _selectedFolder;
        private set { if (Set(ref _selectedFolder, value)) { Raise(nameof(CurrentFolderName)); } }
    }

    private string _activeZone = MailcalBindingsMethods.DeviceTimeZone();
    /// <summary>The active display zone (an IANA id) the rows are localised/ordered in.</summary>
    public string ActiveZone
    {
        get => _activeZone;
        private set { if (Set(ref _activeZone, value)) { Raise(nameof(KeepZoneText)); } }
    }

    private string? _pendingDeviceZone;
    /// <summary>A device zone awaiting the user's adopt/dismiss choice, or <c>null</c>.</summary>
    public string? PendingDeviceZone
    {
        get => _pendingDeviceZone;
        private set
        {
            if (Set(ref _pendingDeviceZone, value))
            {
                Raise(nameof(HasPendingZone));
                Raise(nameof(ZonePromptText));
                Raise(nameof(KeepZoneText));
            }
        }
    }

    // The outgoing-send hint, pulled on a Surface.Sending change: Sending while a send is in
    // flight, then the terminal Sent/Failed. The core owns the terminal -> Idle auto-clear
    // (and its staleness guard), delivering the reset as a later Surface.Sending signal, so
    // this model just publishes whatever SendStatus() reports.
    private SendStatus _sendStatus = SendStatus.Idle;

    /// <summary>Sets the send status and refreshes its bound view helpers.</summary>
    private void UpdateSendStatus(SendStatus status)
    {
        _sendStatus = status;
        Raise(nameof(SendStatusVisible));
        Raise(nameof(SendStatusText));
        Raise(nameof(SendStatusSeverity));
        Raise(nameof(SendStatusBusyVisibility));
    }

    // --- Bindable view helpers (so the XAML needs no converters) --------------

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
    /// The current scope's title: the unified "All Inboxes" (no account selected), else the
    /// selected folder's name or the account's "All Mail".
    /// </summary>
    public string CurrentFolderName
    {
        get
        {
            if (SelectedAccount is null)
            {
                return L10n.SidebarAllInboxes();
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

    /// <summary>The mailbox footer count ("N messages" / "N conversations"), the folder's
    /// full total, not the visible window (<see cref="Rows"/> holds only the loaded page).</summary>
    public string MailCountText =>
        Mode == ViewModeKind.Threaded
            ? L10n.MailboxCountConversations((int)_total)
            : L10n.MailboxCountMessages((int)_total);

    // --- Lifecycle ------------------------------------------------------------

    // The account lifecycle (Start + the connect/add-account orchestration) lives in
    // MailboxModel.Accounts.cs to keep this file within the 500-line limit.

    // Watch for the OS reporting a different time zone (e.g. a laptop changing regions) and
    // forward it to the core, which raises a pending change the UI prompts on, the Windows
    // counterpart of macOS's observeSystemTimeZone(). Two signals are needed: a clock-bearing
    // zone change broadcasts WM_TIMECHANGE (TimeChanged), but a pure region change that leaves
    // the UTC instant unchanged arrives only as a locale setting change (UserPreferenceChanged
    // with Locale), so we listen for both. The core ignores a report of the unchanged zone.
    private void ObserveSystemTimeZone()
    {
        if (_watchingTimeZone)
        {
            return;
        }
        _watchingTimeZone = true;
        SystemEvents.TimeChanged += OnSystemTimeChanged;
        SystemEvents.UserPreferenceChanged += OnUserPreferenceChanged;
    }

    private void OnUserPreferenceChanged(object sender, UserPreferenceChangedEventArgs e)
    {
        if (e.Category == UserPreferenceCategory.Locale)
        {
            OnSystemTimeChanged(sender, EventArgs.Empty);
        }
    }

    // SystemEvents fires on its own thread; .NET caches the local zone, so it is cleared
    // (like macOS's NSTimeZone.resetSystemTimeZone()) before re-reading and reporting on the
    // UI thread. Reporting the unchanged zone is a no-op in the core, so an unrelated clock or
    // locale change is harmless.
    private void OnSystemTimeChanged(object? sender, EventArgs e) =>
        // Shared Rust detection reads the OS fresh each call (region-aware, no .NET zone
        // cache to clear), so the reported zone is the real current city.
        _ui.TryEnqueue(() => ReportDeviceTimeZone(MailcalBindingsMethods.DeviceTimeZone()));

    // The fire-and-forget host intents (mail/calendar actions, navigation, pagination,
    // timezone, reset) and the small account-form helpers live in MailboxModel.Intents.cs,
    // and the snapshot projection (Reload + the identity-preserving reconcile) in
    // MailboxModel.Projection.cs, each split out to keep this file under the 500-line limit.

    // --- INotifyPropertyChanged ----------------------------------------------

    /// <inheritdoc/>
    public event PropertyChangedEventHandler? PropertyChanged;

    private void Raise([CallerMemberName] string? name = null) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));

    private bool Set<T>(ref T field, T value, [CallerMemberName] string? name = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value))
        {
            return false;
        }
        field = value;
        Raise(name);
        return true;
    }
}
