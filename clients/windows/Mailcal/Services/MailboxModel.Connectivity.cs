// Connectivity: the folder pane's footer status, drawn from the notices (docs/notices.md,
// "Signals"), plus the OS network-reachability watch that feeds the core back. Split out of
// MailboxModel.cs to keep that file under the 500-line limit.
//
// What a person is told is the notices themselves (MailboxModel.Notices.cs); what is kept here is
// the footer status, and which accounts have a sign-in Settings asks for again.

using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;
using Windows.Networking.Connectivity;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private bool _offline;
    private List<ConnectionIssue> _connectionIssues = new();
    /// <summary>One account that can't reach its server: its id, display email, and the technical
    /// detail (its connect error) revealed behind the footer status.</summary>
    private sealed record ConnectionIssue(string Id, string Email, string? Detail);

    /// <summary>Reads the footer status, the expired sign-ins and the Allodia card's state from
    /// <paramref name="notices"/>, on every notices signal: an account's connect error can move
    /// while its notice says the same.</summary>
    private void UpdateConnectionStatus(IReadOnlyList<Notice> notices)
    {
        _offline = NoticeRules.Offline(notices);
        _connectionIssues = NoticeRules.Unreachable(notices).Select(id =>
        {
            var email = Accounts.FirstOrDefault(a => a.Id == id)?.Email ?? id;
            return new ConnectionIssue(id, email, _app?.ConnectionDetail(id));
        }).ToList();
        UpdateAllodiaSignInEnded(NoticeRules.AllodiaSignInEnded(notices));
        RaiseConnectionStatus();
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
