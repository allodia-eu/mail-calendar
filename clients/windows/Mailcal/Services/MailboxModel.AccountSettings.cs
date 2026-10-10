// Settings → Accounts' traffic with the core: the one snapshot every account is drawn from, and the
// changes an account's page makes (docs/accounts.md rules 11 to 14). Each change may touch the
// Credential Manager, delete local data or dial a server, so it runs off the UI thread and answers
// with what the page should say.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>How signing an account in again ended. A cancelled one carries neither: the person
/// closed it, and nothing is said.</summary>
internal sealed record ConsentEnded(bool SignedIn, string? Error);

public sealed partial class MailboxModel
{
    /// <summary>Raised when what Settings → Accounts draws may have changed: the core's
    /// settings signal, which adding, removing, linking and signing an account in again raise,
    /// and its connectivity signal, which an expired sign-in and a refused calendar raise.</summary>
    public event Action? AccountsChanged;

    private void RaiseAccountsChanged() => AccountsChanged?.Invoke();

    /// <summary>Every account, mail or not, as the core lists it; empty before the core is up.</summary>
    internal AccountsSnapshot AccountsSnapshot() =>
        _app?.AccountsSnapshot() ?? new AccountsSnapshot([]);

    /// <summary>The accounts whose stored sign-in the server no longer accepts.</summary>
    internal IReadOnlyList<string> SignInExpiredIds => _signInExpired.Select(account => account.Id).ToList();

    /// <summary>Which sign-in an account's credential is, for the remedy its page offers.</summary>
    internal AccountProvider? ProviderOf(string accountId) => _app?.AccountProvider(accountId);

    /// <summary>Switches one of an account's uses on or off.</summary>
    /// <returns>What the core answered, or why it refused.</returns>
    internal async Task<(UseChangeOutcome Outcome, string? Error)> SetAccountUseAsync(
        string accountId, AccountCapability capability, bool on)
    {
        if (_app is not { } app)
        {
            return (UseChangeOutcome.Applied, L10n.AppUnavailable());
        }
        try
        {
            // Blocking: switching a use off deletes what the device holds of it.
            var change = await Task.Run(() => app.SetAccountCapability(accountId, capability, on));
            return (AccountSettingsRules.Outcome(change), null);
        }
        catch (Exception ex)
        {
            Log.Error($"switching an account's use failed: {CoreError.Describe(ex)}");
            return (UseChangeOutcome.Applied, CoreError.Describe(ex));
        }
    }

    /// <summary>Links an account to another for one slot, or clears that link.</summary>
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
            return CoreError.Describe(ex);
        }
    }

    /// <summary>Tries an account's edited servers and sign-in, and keeps them if they connect.</summary>
    /// <returns>Null when saved, otherwise why not; the form is then left as the person typed it.</returns>
    internal async Task<string?> SaveAccountEndpointsAsync(string accountId, AccountEndpoints endpoints)
    {
        if (_app is not { } app)
        {
            return L10n.AppUnavailable();
        }
        try
        {
            await Task.Run(() => app.UpdateAccountEndpoints(accountId, endpoints));
            return null;
        }
        catch (MailcalException.CertificateRejected refused)
        {
            // The message carries the whole certificate; the reason is what a person reads.
            Log.Error("editing an account's servers was refused for a certificate");
            return refused.reason;
        }
        catch (Exception ex)
        {
            Log.Error($"editing an account's servers failed: {CoreError.Describe(ex)}");
            return CoreError.Describe(ex);
        }
    }

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
            return CoreError.Describe(ex);
        }
    }

    /// <summary>Removes an account from Settings. The last one gone returns the window to
    /// first-run setup (docs/accounts.md rule 11).</summary>
    /// <returns>Null when removed, otherwise why not.</returns>
    internal Task<string?> RemoveAccountFromSettingsAsync(string accountId) => RemoveAccountAsync(accountId);

    /// <summary>
    /// Signs an existing Microsoft or Google account in again at its provider, asking for what it
    /// is used for plus <paramref name="adding"/>. The account keeps its id, settings, links and
    /// downloaded mail (docs/provider-oauth.md rule 11), so this never touches the setup form.
    /// </summary>
    /// <remarks>
    /// Through the shared <see cref="SignInFlight"/>: a sign-in abandoned in the browser cannot be
    /// told from one in progress, so the next request supersedes it rather than waiting on it.
    /// </remarks>
    internal Task<ConsentEnded> SignInAgainAsync(string accountId, AccountCapability[] adding)
    {
        if (_connecting || _app is null)
        {
            return Task.FromResult(new ConsentEnded(false, L10n.AppUnavailable()));
        }
        return _signIn.RunAsync(cancel => ConsentFlowAsync(_app, accountId, adding, cancel));
    }

    private async Task<ConsentEnded> ConsentFlowAsync(
        MailcalApp app, string accountId, AccountCapability[] adding, CancellationToken cancel)
    {
        IsSigningIn = true;
        try
        {
            string callbackUrl;
            AccountConsentStart start;
            if (app.AccountProvider(accountId) == AccountProvider.Google)
            {
                using var loopback = new GoogleLoopback();
                start = app.BeginAccountConsent(accountId, loopback.RedirectUri, adding);
                await Windows.System.Launcher.LaunchUriAsync(new Uri(start.AuthorizationUrl));
                callbackUrl = await WaitForGoogleCallbackAsync(loopback, cancel);
            }
            else
            {
                using var callback = ProtocolAuthCallback.Expect(MicrosoftOAuthConfig.CallbackHost);
                start = app.BeginAccountConsent(accountId, MicrosoftOAuthConfig.RedirectUri, adding);
                await Windows.System.Launcher.LaunchUriAsync(new Uri(start.AuthorizationUrl));
                callbackUrl = await callback.WaitAsync(cancel);
            }
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
        catch (Exception ex)
        {
            Log.Error($"signing an account in again failed: {CoreError.Describe(ex)}");
            return new ConsentEnded(false, CoreError.Describe(ex));
        }
        finally
        {
            IsSigningIn = false;
        }
    }
}
