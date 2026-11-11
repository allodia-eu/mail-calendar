// Settings → Accounts' traffic with the core: the one snapshot every account is drawn from, and the
// changes an account's page makes (docs/accounts.md rules 11 to 14). A use, a link, a removal and
// a browser sign-in are the core's to run (docs/waiting.md): dispatched, they answer at once and
// the page says they run. Every other change may touch the Credential Manager or dial a server, so it runs off the UI
// thread and answers with what the page should say.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>Raised when what Settings → Accounts draws may have changed: the core's
    /// settings signal, which adding, removing, linking and signing an account in again raise,
    /// its account-page signal, which a change on a page raises as it starts and ends, and its
    /// notices signal, which an expired sign-in and a refused calendar raise.</summary>
    public event Action? AccountsChanged;

    /// <summary>Raised on the UI thread, once, with what a removal came to when the core says it
    /// needs saying: its stored sign-in could not be erased, so it comes back at the next launch.
    /// Said on Settings → Accounts while it is open, and by the window otherwise.</summary>
    public event Action<string>? RemovalFailed;

    // Whether there was an account when the core last signalled: the last one going is a first run.
    private bool _hadAccounts;

    // The last of the core's change notices said, so each is said once.
    private ulong _accountNoticeSaid;

    private void RaiseAccountsChanged()
    {
        // The last account gone, mail or not, is a first run again (docs/accounts.md rule 11),
        // decided here rather than from the mailbox: an account without mail is in none of its rows.
        var any = AccountsSnapshot().Accounts.Length > 0;
        if (!any && _hadAccounts)
        {
            Log.Info("accounts: the last account is gone; returning to first-run setup");
            NeedsSetup = true;
        }
        _hadAccounts = any;
        if (_app?.AccountsNotice() is { } raised && raised.Id > _accountNoticeSaid)
        {
            _accountNoticeSaid = raised.Id;
            if (raised.Notice is AccountChangeNotice.RemoveFailed failed)
            {
                Log.Error("accounts: a removed account's stored sign-in could not be erased");
                RemovalFailed?.Invoke(L10n.RemoveAccountFailed(failed.Detail));
            }
        }
        DriveBrowserSignIn();
        AccountsChanged?.Invoke();
        RefreshSetupLinks();
        RefreshSetupSeparateServer();
    }

    /// <summary>The page for <paramref name="accountId"/> as the core decided it: its uses, links
    /// and removal. Null once the account is gone.</summary>
    internal AccountPage? AccountPageOf(string accountId) => _app?.AccountPage(accountId);

    /// <summary>What a change on <paramref name="page"/> came to, the first time it is asked;
    /// null when there is nothing new to say.</summary>
    internal AccountChangeNotice? TakeChangeNotice(AccountPage? page)
    {
        if (page?.Notice is not { } raised || raised.Id <= _accountNoticeSaid)
        {
            return null;
        }
        _accountNoticeSaid = raised.Id;
        Log.Info($"accounts: a change on an account's page has something to say ({raised.Notice.GetType().Name})");
        return raised.Notice;
    }

    /// <summary>Asks the core for a change on an account's page; it answers at once, and the page
    /// says it runs.</summary>
    internal void ChangeAccount(AccountPageIntent intent) => _app?.Dispatch(new Intent.AccountPage(intent));

    /// <summary>Every account, mail or not, as the core lists it; empty before the core is up.</summary>
    internal AccountsSnapshot AccountsSnapshot() =>
        _app?.AccountsSnapshot() ?? new AccountsSnapshot([]);

    /// <summary>Switches one of an account's uses on or off, for setup's own steps, which wait on
    /// the answer.</summary>
    /// <returns>Why the core refused, or null; a use it could not switch on is logged.</returns>
    internal async Task<string?> SetAccountUseAsync(string accountId, AccountCapability capability, bool on)
    {
        if (_app is not { } app)
        {
            return L10n.AppUnavailable();
        }
        try
        {
            // Blocking: switching a use off deletes what the device holds of it.
            var change = await Task.Run(() => app.SetAccountCapability(accountId, capability, on));
            if (change != CapabilityChange.Applied)
            {
                Log.Warn($"switching an account's use was not applied: {change}");
            }
            return null;
        }
        catch (Exception ex)
        {
            Log.Error($"switching an account's use failed: {CoreError.Describe(ex)}");
            return CoreErrorCopy.Said(ex);
        }
    }

    /// <summary>Links an account to another for one slot, or clears that link, for setup's link
    /// step, which stores its picks one after another.</summary>
    /// <returns>Null when stored, otherwise why not.</returns>
    internal async Task<string?> SetAccountLinkAsync(string accountId, LinkSlot slot, string? target)
    {
        if (_app is not { } app)
        {
            return L10n.AppUnavailable();
        }
        try
        {
            // Stored through the Credential Manager, which may block.
            await Task.Run(() => app.SetAccountLink(accountId, slot, target));
            return null;
        }
        catch (Exception ex)
        {
            Log.Error($"linking an account failed: {CoreError.Describe(ex)}");
            return CoreErrorCopy.Said(ex);
        }
    }

    /// <summary>An account's page opened: its server is asked again, on the core's own thread,
    /// which mailboxes its sign-in opens besides its own, and the page is signalled once it
    /// answers.</summary>
    internal void AccountPageOpened(string accountId) => _app?.AccountPageOpened(accountId);

    /// <summary>What <paramref name="endpoints"/> leaves out of an account's servers, as its save
    /// would refuse it. Reads only what is stored, so the page asks it as the person types.</summary>
    internal AccountEndpointsGap? EndpointsGap(string accountId, AccountEndpoints endpoints) =>
        _app?.AccountEndpointsGap(accountId, endpoints);
}
