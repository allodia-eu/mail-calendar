// Connectivity: the device-wide offline state and the per-account "can't reach its server"
// outages the core reports on Surface.Connectivity, plus the OS network-reachability watch that
// feeds the core back. Split out of MailboxModel.cs to keep that file under the 500-line limit.
//
// Two distinct conditions, deliberately not merged: the DEVICE has no network (one banner, the
// mail on screen is the last-synced copy), or SOME ACCOUNTS can't reach their server while the
// device is online (a provider outage or a stale password, a per-account banner naming them).

using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;
using Windows.Networking.Connectivity;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private bool _offline;
    private HashSet<string> _unreachableAccounts = new();
    private List<ConnectionIssue> _connectionIssues = new();
    // Friendly emails of Microsoft accounts whose calendar is withheld for lack of the calendar
    // OAuth scope (connected before calendar support, or revoked consent), drives the calendar
    // re-auth banner. Reconnecting an account re-runs its sign-in with the calendar scope.
    private List<string> _calendarReauthEmails = new();
    // Friendly emails of Microsoft accounts whose mail write/send is withheld for lack of the
    // Mail.ReadWrite / Mail.Send OAuth scopes (connected before those scopes, or revoked consent),
    // so a send or mail action was refused, drives the mail re-auth banner. Reconnecting re-runs
    // sign-in with the full scope set, clearing this and any calendar prompt at once.
    private List<string> _mailReauthEmails = new();
    // Accounts whose stored sign-in the server has stopped accepting, an expired or revoked OAuth
    // grant (Google invalid_grant, a Microsoft AADSTS700082), or a password it now refuses. Nothing
    // syncs and a retry never helps, so this drives its own banner rather than the connection-issues
    // one; the core already keeps such an account out of the unreachable list.
    private List<ExpiredSignIn> _signInExpired = new();

    /// <summary>One account that can't reach its server: its id, display email, and the technical
    /// detail (its connect error) revealed behind the banner / footer "details" link.</summary>
    private sealed record ConnectionIssue(string Id, string Email, string? Detail);

    /// <summary>One account whose stored sign-in the server has stopped accepting: its account id,
    /// display email and provider family, which decides the remedy, Microsoft/Google/OAuth-JMAP
    /// re-run their browser sign-in, anything else is updated in Settings. The id is what a JMAP
    /// re-authentication is addressed to: it re-authorises that account's own persisted grant
    /// rather than starting a discovery from an address.</summary>
    private sealed record ExpiredSignIn(string Id, string Email, AccountProvider? Provider);

    // Set when a re-authentication launched from the banner failed, so the banner can say so
    // instead of leaving the click looking like it did nothing. Cleared on the next attempt; a
    // success clears the whole banner, since the core retracts the prompt.
    private bool _signInReauthFailed;

    /// <summary>Records the latest connectivity and refreshes its bound view helpers, then
    /// raises <see cref="ConnectivityChanged"/> so the sidebar can re-badge accounts (the nav
    /// items are built imperatively, not data-bound to a per-account flag).</summary>
    private void UpdateConnectivity(ConnectivitySnapshot connectivity)
    {
        _offline = connectivity.Offline;
        _unreachableAccounts = new HashSet<string>(connectivity.UnreachableAccounts);
        // Resolve each unreachable id to its switcher email + the core's technical detail, so the
        // banner can name the affected accounts and reveal the raw error behind a "details" link.
        _connectionIssues = connectivity.UnreachableAccounts.Select(id =>
        {
            var email = Accounts.FirstOrDefault(a => a.Id == id)?.Email ?? id;
            return new ConnectionIssue(id, email, _app?.ConnectionDetail(id));
        }).ToList();
        // A standing permission gap, not a connectivity fault, resolved to emails the same way,
        // and (unlike unreachable) shown regardless of the offline state.
        _calendarReauthEmails = connectivity.CalendarReauthAccounts
            .Select(id => Accounts.FirstOrDefault(a => a.Id == id)?.Email ?? id)
            .ToList();
        // Likewise the mail write/send permission gap (a refused send or mail action).
        _mailReauthEmails = connectivity.MailReauthAccounts
            .Select(id => Accounts.FirstOrDefault(a => a.Id == id)?.Email ?? id)
            .ToList();
        // A dead sign-in, carrying its provider so the button can launch the right flow.
        _signInExpired = connectivity.SigninExpiredAccounts
            .Select(id => new ExpiredSignIn(
                id,
                Accounts.FirstOrDefault(a => a.Id == id)?.Email ?? id,
                _app?.AccountProvider(id)))
            .ToList();
        Raise(nameof(IsOffline));
        Raise(nameof(HasConnectionIssues));
        Raise(nameof(ConnectionIssuesText));
        Raise(nameof(ConnectionIssuesDetail));
        Raise(nameof(HasCalendarReauth));
        Raise(nameof(CalendarReauthText));
        Raise(nameof(HasMailReauth));
        Raise(nameof(MailReauthText));
        Raise(nameof(HasSignInExpired));
        Raise(nameof(SignInExpiredText));
        Raise(nameof(CanRelaunchSignIn));
        Raise(nameof(SignInExpiredActionVisible));
        RaiseConnectionStatus();
        ConnectivityChanged?.Invoke();
    }

    /// <summary>Whether the device has no network (drives the offline banner).</summary>
    public bool IsOffline => _offline;

    /// <summary>Whether <paramref name="accountId"/>'s server couldn't be reached on its last
    /// sync while online, a per-account outage, distinct from the device-wide offline state.</summary>
    public bool IsAccountUnreachable(string accountId) => _unreachableAccounts.Contains(accountId);

    /// <summary>Raised when connectivity changes, so the imperatively-built sidebar can re-badge.</summary>
    public event Action? ConnectivityChanged;

    /// <summary>Whether any account can't reach its server (while online), drives the top
    /// connection banner. Empty while the whole device is offline (the offline banner stands in).</summary>
    public bool HasConnectionIssues => _connectionIssues.Count > 0;

    /// <summary>The friendly banner text naming the accounts that can't connect.</summary>
    public string ConnectionIssuesText =>
        L10n.ConnectivityAccountsAffected(string.Join(", ", _connectionIssues.Select(i => i.Email)));

    /// <summary>The joined technical errors behind the banner's "Details" link, one per affected
    /// account (the core already prefixes each line with its account address).</summary>
    public string ConnectionIssuesDetail =>
        string.Join("\n\n", _connectionIssues.Select(i => i.Detail ?? i.Email));

    /// <summary>Whether a Microsoft account's calendar is withheld for lack of the calendar OAuth
    /// scope, drives the calendar re-auth banner. Mail is unaffected; reconnecting grants it.</summary>
    public bool HasCalendarReauth => _calendarReauthEmails.Count > 0;

    /// <summary>The calendar re-auth banner text, naming the affected account(s).</summary>
    public string CalendarReauthText =>
        L10n.CalendarReauthPrompt(string.Join(", ", _calendarReauthEmails));

    /// <summary>The address to re-authenticate when the banner's "Reconnect" is clicked (the first
    /// affected account; the banner re-renders after it clears, walking through any others).</summary>
    public string? CalendarReauthEmail => _calendarReauthEmails.FirstOrDefault();

    /// <summary>Whether a Microsoft account's mail write/send is withheld for lack of the
    /// Mail.ReadWrite / Mail.Send OAuth scopes, drives the mail re-auth banner. Reading is
    /// unaffected; reconnecting re-runs sign-in with the full scope set.</summary>
    public bool HasMailReauth => _mailReauthEmails.Count > 0;

    /// <summary>The mail re-auth banner text, naming the affected account(s).</summary>
    public string MailReauthText =>
        L10n.MailReauthPrompt(string.Join(", ", _mailReauthEmails));

    /// <summary>The address to re-authenticate when the mail banner's "Reconnect" is clicked (the
    /// first affected account; the banner re-renders after it clears, walking through any others).</summary>
    public string? MailReauthEmail => _mailReauthEmails.FirstOrDefault();

    /// <summary>Whether an account's stored sign-in has stopped being accepted, drives the
    /// "sign in again" banner. Not an outage: the server answered, and only a fresh sign-in
    /// helps, so this is deliberately separate from <see cref="HasConnectionIssues"/>.</summary>
    public bool HasSignInExpired => _signInExpired.Count > 0;

    /// <summary>The expired-sign-in banner text, naming the affected account(s). Points at the
    /// button when there is a browser flow to re-run, and at Settings when there isn't, plus, on
    /// a failed attempt, a plain line saying so (the cause is an OAuth protocol string and belongs
    /// in the log, not in front of the user).</summary>
    public string SignInExpiredText
    {
        get
        {
            var names = string.Join(", ", _signInExpired.Select(a => a.Email));
            var prompt = CanRelaunchSignIn
                ? L10n.SigninExpiredPrompt(names)
                : L10n.SigninExpiredPromptSettings(names);
            return _signInReauthFailed ? $"{prompt} {L10n.SigninExpiredFailed()}" : prompt;
        }
    }

    /// <summary>Whether the first affected account has a sign-in this app can re-launch: an OAuth
    /// provider, including a JMAP account that was connected by signing in. A password account,
    /// or a JMAP one holding a pasted password/API token, is re-entered in Settings, so the
    /// button is hidden.</summary>
    public bool CanRelaunchSignIn =>
        _signInExpired.FirstOrDefault()?.Provider
            is AccountProvider.Microsoft or AccountProvider.Google or AccountProvider.JmapOauth;

    /// <summary>The banner button's visibility, bound directly, as WinUI has no built-in
    /// bool-to-Visibility conversion for <c>x:Bind</c>.</summary>
    public Visibility SignInExpiredActionVisible =>
        CanRelaunchSignIn ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>Signs the first affected account back in, launching the flow its provider needs
    /// (the banner re-renders for the next after each clears). A no-op for a password account, or
    /// a JMAP one holding a pasted secret, whose button is hidden, there is no browser flow to
    /// run, and the message points at Settings instead. The provider choice stays in here rather
    /// than on a public property because the generated <c>AccountProvider</c> enum is internal to
    /// the bindings.</summary>
    public void ReconnectExpiredSignIn()
    {
        if (_signInExpired.FirstOrDefault() is not { } account)
        {
            return;
        }
        SetSignInReauthFailed(false);
        switch (account.Provider)
        {
            case AccountProvider.Microsoft:
                SignInWithMicrosoft(account.Email);
                break;
            case AccountProvider.Google:
                SignInWithGoogle(account.Email);
                break;
            case AccountProvider.JmapOauth:
                // Addressed to the account id, not the address: the core re-authorises this
                // account's own stored grant, so there is no discovery and no second registration.
                _ = ReconnectJmapAsync(account.Id);
                break;
        }
    }

    /// <summary>Records whether the last banner-launched re-authentication failed, and refreshes
    /// the banner text that reports it.</summary>
    private void SetSignInReauthFailed(bool failed)
    {
        _signInReauthFailed = failed;
        Raise(nameof(SignInExpiredText));
    }

    // The unreachable accounts within the current scope: the selected account only, or all of them
    // in the unified inbox. Drives the folder-pane footer's connection-status label + its flyout.
    private IReadOnlyList<ConnectionIssue> ScopeIssues =>
        SelectedAccount is { } selected
            ? _connectionIssues.Where(i => i.Id == selected).ToList()
            : _connectionIssues;

    /// <summary>Whether the current scope is fully connected (drives the footer status label).</summary>
    public bool ConnectionHealthy => !_offline && ScopeIssues.Count == 0;

    /// <summary>The folder-pane footer connection-status label ("Verbonden" / "Niet verbonden").</summary>
    public string ConnectionStatusText =>
        ConnectionHealthy ? L10n.ConnectivityConnected() : L10n.ConnectivityNotConnected();

    /// <summary>Show the caution glyph beside the footer status only when not connected.</summary>
    public Visibility ConnectionStatusWarningVisibility =>
        ConnectionHealthy ? Visibility.Collapsed : Visibility.Visible;

    /// <summary>The detail shown in the footer status flyout for the current scope: the offline
    /// notice, the affected accounts' errors, or a plain "connected" line.</summary>
    public string ConnectionStatusDetail
    {
        get
        {
            if (_offline)
            {
                return L10n.ConnectivityOfflineBanner();
            }
            if (ScopeIssues.Count == 0)
            {
                return L10n.ConnectivityConnected();
            }
            return string.Join("\n\n", ScopeIssues.Select(i => i.Detail ?? i.Email));
        }
    }

    // Refreshes every bound helper derived from the connection status (the footer label + flyout),
    // after a connectivity change or a scope (selected-account) change.
    private void RaiseConnectionStatus()
    {
        Raise(nameof(ConnectionHealthy));
        Raise(nameof(ConnectionStatusText));
        Raise(nameof(ConnectionStatusWarningVisibility));
        Raise(nameof(ConnectionStatusDetail));
    }

    private bool _watchingNetwork;

    /// <summary>Watches the device's network reachability and forwards each change to the core
    /// (ReportNetworkReachable): offline stops it attempting syncs (and raises the banner), online
    /// triggers a catch-up refresh that also re-dials any dropped provider connections.</summary>
    private void ObserveNetworkReachability()
    {
        if (_watchingNetwork)
        {
            return;
        }
        _watchingNetwork = true;
        NetworkInformation.NetworkStatusChanged += OnNetworkStatusChanged;
        // NetworkStatusChanged doesn't fire on subscribe, so report the current state once.
        ReportNetworkReachable();
    }

    // NetworkInformation fires on its own thread; hop to the UI thread before touching the core.
    // Reporting the unchanged value is a no-op in the core, so a spurious re-signal is harmless.
    private void OnNetworkStatusChanged(object sender) => _ui.TryEnqueue(ReportNetworkReachable);

    private void ReportNetworkReachable()
    {
        var reachable = NetworkInformation.GetInternetConnectionProfile() is not null;
        _app?.Dispatch(new Intent.ReportNetworkReachable(reachable));
    }
}
