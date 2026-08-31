// Sync progress: the two things the core lets a client say about mail arriving, kept apart on
// purpose. The BAR is a download the user is waiting on, adding an account, opening an unsynced
// folder, an explicit refetch, and is the only one allowed a row of layout. The HINT is a pass
// nobody asked for (a poll tick, a push, a boot catch-up): it names the accounts currently
// pulling mail down, inside the status line the footer already draws, so the list never moves for
// work the user did not start.
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

    /// <summary>Records the latest sync progress, the awaited download and the background
    /// hint, and refreshes their bound view helpers.</summary>
    private void UpdateSyncProgress(SyncProgressSnapshot progress)
    {
        _syncActive = progress.Active;
        _syncFetched = progress.Fetched;
        _syncTotal = progress.Total;
        _syncingAccounts = progress.Accounts;
        Raise(nameof(SyncProgressVisible));
        Raise(nameof(SyncProgressText));
        Raise(nameof(SyncProgressIsIndeterminate));
        Raise(nameof(SyncProgressValue));
        Raise(nameof(SyncProgressMaximum));
        Raise(nameof(SyncHintVisible));
        Raise(nameof(SyncHintText));
        Raise(nameof(SyncHintColumnWidth));
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

    /// <summary>Whether a background sync is downloading mail right now (drives the footer
    /// hint). Collapsed whenever nothing is arriving unasked, which is almost always, the core
    /// admits an account only once its background pass has actually committed mail, so a poll
    /// that finds nothing shows nothing.</summary>
    public Visibility SyncHintVisible =>
        _syncingAccounts.Count > 0 ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>The width of the status line's middle column, which holds the hint: elastic while
    /// the hint is up, so a hint too long for the row trims instead of shoving the connection
    /// status off screen, and zero otherwise, so the status sits beside the message count exactly
    /// as it does when nothing is syncing.</summary>
    public GridLength SyncHintColumnWidth => _syncingAccounts.Count > 0
        ? new GridLength(1, GridUnitType.Star)
        : new GridLength(0, GridUnitType.Auto);

    /// <summary>The footer's background-sync hint: which accounts are pulling mail down, and how
    /// far through their folders they are. A caption in the status line the footer already draws,
    /// never a bar, a pass the user did not start may not take a row of layout and move the
    /// list. Empty when the hint is collapsed.</summary>
    public string SyncHintText
    {
        get
        {
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
            // Named from the app's own account list, which is where every other surface gets the
            // address; the id is a fallback for an account removed mid-pass.
            var name = Accounts.FirstOrDefault(a => a.Id == only.AccountId)?.Email ?? only.AccountId;
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

    private static string Count(ulong value) => value.ToString("N0", CultureInfo.CurrentCulture);
}
