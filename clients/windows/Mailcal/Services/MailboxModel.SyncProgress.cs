// Sync progress: the three things the core lets a client say about mail arriving, kept apart on
// purpose. The BAR is a download the user is waiting on, adding an account, opening an unsynced
// folder, an explicit refetch, and is the only one allowed a row of layout. The HINT is a pass
// nobody asked for (a poll tick, a push, a boot catch-up): it names the accounts currently
// pulling mail down, inside the status line the footer already draws, so the list never moves for
// work the user did not start. The PAUSE is an account whose server asked to be left alone for a
// while: nothing is arriving for it and nothing is wrong, which is the one combination the other
// two cannot express. It takes the status line ahead of the hint, as a short label with the whole
// sentence behind a hover.
//
// Split out of MailboxModel.cs to keep that file under the 500-line limit.

using System.Globalization;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private bool _syncActive;
    private ulong _syncFetched;
    private ulong? _syncTotal;
    private IReadOnlyList<AccountSyncProgress> _syncingAccounts = [];
    private IReadOnlyList<ThrottledAccount> _pausedAccounts = [];

    /// <summary>Records the latest sync progress, the awaited download and the background
    /// hint, and refreshes their bound view helpers.</summary>
    private void UpdateSyncProgress(SyncProgressSnapshot progress)
    {
        _syncActive = progress.Active;
        _syncFetched = progress.Fetched;
        _syncTotal = progress.Total;
        _syncingAccounts = progress.Accounts;
        _pausedAccounts = progress.Throttled;
        Raise(nameof(SyncProgressVisible));
        Raise(nameof(SyncProgressText));
        Raise(nameof(SyncProgressIsIndeterminate));
        Raise(nameof(SyncProgressValue));
        Raise(nameof(SyncProgressMaximum));
        Raise(nameof(SyncStatusVisible));
        Raise(nameof(SyncStatusText));
        Raise(nameof(SyncStatusDetail));
        Raise(nameof(SyncStatusSpoken));
        Raise(nameof(SyncStatusColumnWidth));
    }

    /// <summary>Whether a background mail download is in progress (drives the bar's visibility).</summary>
    public Visibility SyncProgressVisible => _syncActive ? Visibility.Visible : Visibility.Collapsed;
    /// <summary>Whether the total isn't known yet, so the bar runs indeterminate.</summary>
    public bool SyncProgressIsIndeterminate => _syncTotal is null;
    /// <summary>The bar's maximum (the in-window total), or 1 while indeterminate.</summary>
    public double SyncProgressMaximum => _syncTotal is ulong total && total > 0 ? total : 1;
    /// <summary>The bar's current value (messages committed so far this pass).</summary>
    public double SyncProgressValue => _syncFetched;
    /// <summary>The "downloading Y of X" caption beside the bar (thousands-separated).</summary>
    public string SyncProgressText => _syncTotal is ulong total
        ? L10n.SyncDownloading(Count(_syncFetched), Count(total))
        : L10n.SyncDownloadingIndeterminate(Count(_syncFetched));

    /// <summary>Whether the status line has anything to say: an account a server has asked to
    /// wait, or a background sync downloading mail. Collapsed whenever it has not, which is
    /// almost always, the core admits an account to the hint only once its pass has actually
    /// committed mail, so a poll that finds nothing shows nothing.</summary>
    public Visibility SyncStatusVisible =>
        HasSyncStatus ? Visibility.Visible : Visibility.Collapsed;

    private bool HasSyncStatus => _pausedAccounts.Count > 0 || _syncingAccounts.Count > 0;

    /// <summary>The width of the status line's middle column, which holds the caption: elastic
    /// while it is up, so a caption too long for the row trims instead of shoving the connection
    /// status off screen, and zero otherwise, so the status sits beside the message count exactly
    /// as it does when nothing is syncing.</summary>
    public GridLength SyncStatusColumnWidth => HasSyncStatus
        ? new GridLength(1, GridUnitType.Star)
        : new GridLength(0, GridUnitType.Auto);

    /// <summary>The footer's caption. A pause takes the line ahead of the hint: a pass that is
    /// downloading is already evident from the list filling, and a pass that is waiting is
    /// evident from nothing at all. The core keeps the two sets disjoint, so this only orders
    /// them. Empty when the line is collapsed.</summary>
    public string SyncStatusText
    {
        get
        {
            if (_pausedAccounts.Count > 0)
            {
                return L10n.SyncPaused();
            }
            if (_syncingAccounts.Count == 0)
            {
                return string.Empty;
            }
            // Several at once carry no counts: one account in its folders and another in its
            // bodies have no shared unit to add up, and a status line cannot name them all anyway.
            if (_syncingAccounts.Count > 1)
            {
                return L10n.SyncHintAccounts(_syncingAccounts.Count);
            }
            var only = _syncingAccounts[0];
            var name = AccountName(only.AccountId);
            if (only.WarmingBodies)
            {
                return L10n.SyncHintBodies(name, Count(only.BodiesDone));
            }
            return L10n.SyncHintAccount(
                name,
                only.FoldersDone.ToString(CultureInfo.CurrentCulture),
                only.FoldersTotal.ToString(CultureInfo.CurrentCulture));
        }
    }

    /// <summary>The whole sentence behind the caption's hover, for the paused notice whose
    /// caption is a short label standing in for one. Null where the caption already says
    /// everything, which leaves no tooltip and lets the TextBlock name itself.</summary>
    ///
    /// <remarks>The wait is stated as an approximation. It was rounded up before it got here, and
    /// nothing re-reads the clock while the tooltip is up.</remarks>
    public string? SyncStatusDetail
    {
        get
        {
            if (_pausedAccounts.Count == 0)
            {
                return null;
            }
            // Several at once are not named, exactly as with the hint: a status line cannot name
            // them all, and their waits have no shared end to state.
            if (_pausedAccounts.Count > 1)
            {
                return L10n.SyncPausedDetailAccounts(_pausedAccounts.Count);
            }
            var only = _pausedAccounts[0];
            var name = AccountName(only.AccountId);
            // Two refusals in three name no instant. "Shortly" is the honest answer; a figure
            // here would be one we made up.
            return only.ResumesInMinutes is uint minutes
                ? L10n.SyncPausedDetail(name, (int)minutes)
                : L10n.SyncPausedDetailSoon(name);
        }
    }

    /// <summary>What a screen reader hears: the whole sentence where there is one, the caption
    /// itself otherwise. Bound explicitly rather than left to the TextBlock's own text, because
    /// "Sync paused" alone never says how long.</summary>
    public string SyncStatusSpoken => SyncStatusDetail ?? SyncStatusText;

    /// <summary>Names an account from the app's own account list, which is where every other
    /// surface gets the address; the id is the fallback for one removed mid-pass.</summary>
    private string AccountName(string id) =>
        Accounts.FirstOrDefault(a => a.Id == id)?.Email ?? id;

    private static string Count(ulong value) => value.ToString("N0", CultureInfo.CurrentCulture);
}
