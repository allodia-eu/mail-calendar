// Settings → Accounts' traffic with the core: the one snapshot every account is drawn from, and the
// changes an account's page makes (docs/accounts.md rules 11 to 14). A use, a link and a removal
// are the core's to run (docs/waiting.md): dispatched, they answer at once and the page says they
// run. Every other change may touch the Credential Manager or dial a server, so it runs off the UI
// thread and answers with what the page should say.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>How signing an account in again ended. A cancelled one carries neither: the person
/// closed it, and nothing is said. <paramref name="Cause"/> is the core's error.
/// <paramref name="Unanswered"/> is a wait for the browser that ran out, which says so but is not
/// a failure: a closed tab ends there too (docs/notices.md rule 10).</summary>
internal sealed record ConsentEnded(bool SignedIn, string? Error, Exception? Cause = null, bool Unanswered = false)
{
    internal static ConsentEnded Failed(Exception error) => new(false, CoreErrorCopy.Said(error), error);

    internal static ConsentEnded TimedOut() => new(false, L10n.AccountSigninTimeout(), Unanswered: true);
}

public sealed partial class MailboxModel
{
    /// <summary>Raised when what Settings → Accounts draws may have changed: the core's
    /// settings signal, which adding, removing, linking and signing an account in again raise,
    /// and its notices signal, which an expired sign-in and a refused calendar raise.</summary>
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
        AccountsChanged?.Invoke();
        RefreshSetupLinks();
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
        Log.Warn("accounts: a change on an account's page did not take");
        return raised.Notice;
    }

    /// <summary>Asks the core for a change on an account's page; it answers at once, and the page
    /// says it runs.</summary>
    internal void ChangeAccount(AccountPageIntent intent) => _app?.Dispatch(new Intent.AccountPage(intent));

    /// <summary>Every account, mail or not, as the core lists it; empty before the core is up.</summary>
    internal AccountsSnapshot AccountsSnapshot() =>
        _app?.AccountsSnapshot() ?? new AccountsSnapshot([]);

    /// <summary>The accounts whose stored sign-in the server no longer accepts.</summary>
    internal IReadOnlyList<string> SignInExpiredIds => _signInExpired;

    /// <summary>Which sign-in an account's credential is, for the remedy its page offers.</summary>
    internal AccountProvider? ProviderOf(string accountId) => _app?.AccountProvider(accountId);

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

    /// <summary>What <paramref name="endpoints"/> leaves out of an account's servers, as its save
    /// would refuse it. Reads only what is stored, so the page asks it as the person types.</summary>
    internal AccountEndpointsGap? EndpointsGap(string accountId, AccountEndpoints endpoints) =>
        _app?.AccountEndpointsGap(accountId, endpoints);

    /// <summary>Replaces a JMAP account's pasted password or API token, after proving it connects.</summary>
    /// <returns>Null when replaced, otherwise why not.</returns>
    internal async Task<string?> ReplaceAccountSecretAsync(string accountId, string secret)
    {
        if (_app is not { } app)
        {
            return L10n.AppUnavailable();
        }
        try
        {
            await Task.Run(() => app.ReplaceAccountSecret(accountId, secret));
            return null;
        }
        catch (Exception ex)
        {
            Log.Error($"replacing an account's secret failed: {CoreError.Describe(ex)}");
            return CoreErrorCopy.Said(ex);
        }
    }

    /// <summary>
    /// Signs an existing Microsoft or Google account in again at its provider, asking for what it
    /// is used for plus <paramref name="adding"/>. The account keeps its id, settings, links and
    /// downloaded mail (docs/provider-oauth.md rule 11), so this never touches the setup form.
    /// </summary>
    /// <remarks>
    /// Through the shared <see cref="SignInFlight"/>: a sign-in abandoned in the browser cannot be
    /// told from one in progress, so the next request supersedes it rather than waiting on it.
    /// </remarks>
    internal Task<ConsentEnded> SignInAgainAsync(
        string accountId, AccountCapability[] adding, Action<SignInStep>? step = null)
    {
        if (_connecting || _app is null)
        {
            return Task.FromResult(new ConsentEnded(false, L10n.AppUnavailable()));
        }
        return _signIn.RunAsync(cancel => ConsentFlowAsync(_app, accountId, adding, step ?? (_ => { }), cancel));
    }

    private async Task<ConsentEnded> ConsentFlowAsync(
        MailcalApp app, string accountId, AccountCapability[] adding, Action<SignInStep> step, CancellationToken cancel)
    {
        IsSigningIn = true;
        try
        {
            string callbackUrl;
            AccountConsentStart start;
            if (app.AccountProvider(accountId) == AccountProvider.Google)
            {
                using var loopback = NewGoogleLoopback();
                start = app.BeginAccountConsent(accountId, loopback.RedirectUri, adding);
                await Windows.System.Launcher.LaunchUriAsync(new Uri(start.AuthorizationUrl));
                step(SignInStep.Browser);
                callbackUrl = await WaitForGoogleCallbackAsync(loopback, cancel);
            }
            else
            {
                using var callback = ProtocolAuthCallback.Expect(MicrosoftOAuthConfig.CallbackHost);
                start = app.BeginAccountConsent(accountId, MicrosoftOAuthConfig.RedirectUri, adding);
                await Windows.System.Launcher.LaunchUriAsync(new Uri(start.AuthorizationUrl));
                step(SignInStep.Browser);
                callbackUrl = await callback.WaitAsync(cancel);
            }
            step(SignInStep.Calling);
            // The exchange, the connect and the catch-up all block.
            await Task.Run(() => app.CompleteAccountConsent(start.Pending, callbackUrl));
            Log.Info("an account was signed in again in place");
            return new ConsentEnded(true, null);
        }
        catch (OperationCanceledException)
        {
            Log.Info("signing an account in again was cancelled");
            return new ConsentEnded(false, null);
        }
        catch (TimeoutException)
        {
            Log.Warn("signing an account in again timed out waiting for the browser");
            return ConsentEnded.TimedOut();
        }
        catch (Exception ex)
        {
            Log.Error($"signing an account in again failed: {CoreError.Describe(ex)}");
            return ConsentEnded.Failed(ex);
        }
        finally
        {
            IsSigningIn = false;
        }
    }
}
