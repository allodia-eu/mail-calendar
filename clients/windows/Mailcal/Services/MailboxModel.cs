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

    /// <summary>The unified Inbox badge: every account's Inbox unread, summed. <c>0</c> shows
    /// none. It belongs on that row rather than on the All Accounts heading above it, which
    /// carries no count for the reason an account row carries none (docs/folder-pane.md, rule 8).</summary>
    public uint UnifiedUnread { get; private set; }

    /// <summary>
    /// Whether the All Accounts group's tree is open in the folder pane. The core's value, and
    /// persisted by it: the pane renders this rather than remembering an answer of its own, which
    /// is what makes it agree with the other platforms and survive a restart
    /// (docs/folder-pane.md, rule 16).
    /// </summary>
    /// <remarks>
    /// It notifies, unlike <see cref="UnifiedUnread"/> beside it, because the pane is reconciled
    /// off a handful of named signals and this one arrives on its own: the persisted value comes
    /// back in the first snapshot **after** the accounts are published, so a shell waiting for a
    /// collection change would read the default and open a group the user had shut.
    /// </remarks>
    public bool UnifiedExpanded
    {
        get => _unifiedExpanded;
        private set => Set(ref _unifiedExpanded, value);
    }

    private bool _unifiedExpanded = true;

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

    private string? _senderNamePrompt;
    /// <summary>
    /// The account whose "your name" step is owed, or <c>null</c> when none is.
    /// </summary>
    /// <remarks>
    /// Set by every route that adds an account, so the step follows a manual connect and a
    /// browser sign-in alike; a route that cannot name the account it added sets nothing, because
    /// asking "your name" without knowing whose would write the answer onto whichever account
    /// happened to be first (docs/sending.md). The shell clears it as it shows the step.
    /// </remarks>
    public string? SenderNamePrompt
    {
        get => _senderNamePrompt;
        set => Set(ref _senderNamePrompt, value);
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
        private set { if (Set(ref _selectedAccount, value)) { RaiseListTitle(); RaiseConnectionStatus(); } }
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
        private set
        {
            if (Set(ref _mode, value))
            {
                Raise(nameof(IsThreaded));
                Raise(nameof(MailCountText));
                Raise(nameof(SelectionPaneText));
            }
        }
    }

    /// <summary>Whether the list is grouped into threads (drives the header toggle).</summary>
    public bool IsThreaded => Mode == ViewModeKind.Threaded;

    private string? _selectedFolder;
    /// <summary>The selected folder's key, or <c>null</c> for the selected account's all-mail view.</summary>
    public string? SelectedFolder
    {
        get => _selectedFolder;
        private set { if (Set(ref _selectedFolder, value)) { RaiseListTitle(); } }
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

    // The bindable view helpers the XAML reads instead of converters (visibilities, the send
    // hint, the list's own title and count) live in MailboxModel.ViewHelpers.cs, to keep this file
    // within the 500-line limit.

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

    /// <summary>Announces the scope's name and the header built on it together, so the two can
    /// never be raised apart.</summary>
    private void RaiseListTitle()
    {
        Raise(nameof(CurrentFolderName));
        Raise(nameof(ListTitle));
    }

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
