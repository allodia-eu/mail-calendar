// How a standards or JMAP account signs in, changed in place (docs/accounts.md rule 17): what its
// server accepts, signing it in through its provider (again, or in place of its password), a
// password in place of its grant, and the credential an add refused as a duplicate, put on the
// account already there. The core connects with the new credential, stores it and replaces the
// account itself; nothing here calls add_account or writes the Credential Manager.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private ExistingAccountOffer? _setupExisting;

    /// <summary>The account an add was just refused for, with the config the add was given: what
    /// the setup form offers to update. It goes with the refusal, so whatever replaces or clears
    /// <see cref="SetupError"/> takes it away.</summary>
    internal ExistingAccountOffer? SetupExisting
    {
        get => _setupExisting;
        private set
        {
            if (Set(ref _setupExisting, value))
            {
                Raise(nameof(SetupUpdateVisibility));
            }
        }
    }

    /// <summary>Shows the offer to update the account already set up.</summary>
    public Microsoft.UI.Xaml.Visibility SetupUpdateVisibility =>
        _setupExisting is null ? Microsoft.UI.Xaml.Visibility.Collapsed : Microsoft.UI.Xaml.Visibility.Visible;

    /// <summary>Which ways in the account's server accepts, for its page's Sign-in section; null
    /// when the core is not up or could not answer. Blocking (it asks the server, and the
    /// provider's metadata), so it runs off the UI thread.</summary>
    internal async Task<AccountSignInOptions?> AccountSignInOptionsAsync(string accountId)
    {
        if (_app is not { } app)
        {
            return null;
        }
        try
        {
            return await Task.Run(() => app.AccountSignInOptions(accountId));
        }
        catch (Exception ex)
        {
            // The core answers every ordinary failure with nothing offered; a panic lands here.
            Log.Warn($"asking how an account can sign in failed: {CoreError.Describe(ex)}");
            return null;
        }
    }

    /// <summary>
    /// Signs a standards or JMAP account in through its provider in the browser: again from its
    /// stored grant when it already signs in that way, in place of its password otherwise.
    /// </summary>
    /// <param name="ask">Asked, before the browser opens, what becomes of the calendar and
    /// contacts a switch from a password will not reach (docs/accounts.md rule 18). Without it
    /// such a switch is abandoned; only Settings, where a switch starts, passes one.</param>
    /// <remarks>
    /// Through the shared <see cref="SignInFlight"/>, for the reason recorded on
    /// <see cref="SignInWithJmapAsync"/>: every protocol-activation flow shares one redirect slot.
    /// </remarks>
    internal Task<ConsentEnded> AccountSignInAsync(
        string accountId, Func<IReadOnlyList<AccountCapability>, Task<UnreachedAnswer>>? ask = null)
    {
        if (_connecting || _app is null)
        {
            return Task.FromResult(new ConsentEnded(false, L10n.AppUnavailable()));
        }
        return _signIn.RunAsync(cancel => AccountSignInFlowAsync(_app, accountId, ask, cancel));
    }

    private async Task<ConsentEnded> AccountSignInFlowAsync(
        MailcalApp app,
        string accountId,
        Func<IReadOnlyList<AccountCapability>, Task<UnreachedAnswer>>? ask,
        CancellationToken cancel)
    {
        IsSigningIn = true;
        try
        {
            // A password account is redirected where setup would send its kind. One that already
            // signs in through its provider replays the redirect its grant was registered with,
            // whichever of the two that was, so the slot waits on both.
            var jmap = AccountsSnapshot().Accounts
                .FirstOrDefault(entry => entry.Id == accountId)?.Kind == AccountKind.Jmap;
            var redirect = jmap ? JmapOAuthConfig.RedirectUri : ImapOAuthConfig.RedirectUri;
            using var callback = ProtocolAuthCallback.Expect(ImapOAuthConfig.CallbackHost, JmapOAuthConfig.CallbackHost);
            // Discovery and registration, for a password account, block.
            var start = await Task.Run(() => app.BeginAccountSignIn(accountId, redirect));
            UnreachedUses? choice = null;
            var unreached = UnreachedUsesRules.Dav(start.Unreached);
            if (UnreachedUsesRules.Asks(unreached))
            {
                // A sign-in started meanwhile supersedes this one, unanswered.
                var answer = ask is null ? UnreachedAnswer.Cancel : await ask(unreached).WaitAsync(cancel);
                if (UnreachedUsesRules.Choice(answer) is not { } chosen)
                {
                    Log.Info("an account's sign-in was abandoned before the browser opened");
                    return new ConsentEnded(false, null);
                }
                choice = chosen;
            }
            if (!await Windows.System.Launcher.LaunchUriAsync(new Uri(start.AuthorizationUrl)))
            {
                Log.Warn("the browser did not open for an account's sign-in");
                return new ConsentEnded(false, L10n.AccountSigninBrowserFailed());
            }
            var callbackUrl = await callback.WaitAsync(cancel);
            // The exchange, the connect and storing the new credential all block.
            await Task.Run(() => app.CompleteAccountSignIn(accountId, start.Pending, callbackUrl, choice));
            Log.Info($"an account was signed in through its provider in place: {LogHandle(accountId)}");
            if (choice == UnreachedUses.KeepOnPassword)
            {
                // The calendar and contacts kept on the password are an account of their own now.
                SyncAfterAccountChange();
            }
            return new ConsentEnded(true, null);
        }
        catch (OperationCanceledException)
        {
            Log.Info("an account's sign-in was cancelled");
            return new ConsentEnded(false, null);
        }
        catch (TimeoutException)
        {
            Log.Warn("an account's sign-in timed out waiting for the browser");
            return new ConsentEnded(false, L10n.AccountSigninTimeout());
        }
        catch (Exception ex)
        {
            Log.Error($"an account's sign-in failed: {CoreError.Describe(ex)}");
            return new ConsentEnded(false, CoreError.Said(ex, L10n.SigninRefused(), L10n.AccountAlreadySetUp(), L10n.SigninDifferentAccount()));
        }
        finally
        {
            IsSigningIn = false;
        }
    }

    /// <summary>Switches an account that signs in through its provider to <paramref name="password"/>,
    /// after the server has accepted it.</summary>
    /// <returns>Null when switched, otherwise why not.</returns>
    internal async Task<string?> SwitchAccountToPasswordAsync(string accountId, string password)
    {
        if (_app is not { } app)
        {
            return L10n.AppUnavailable();
        }
        try
        {
            // A connect with the password, then the store.
            await Task.Run(() => app.SwitchAccountToPassword(accountId, password));
            Log.Info($"an account now signs in with a password: {LogHandle(accountId)}");
            return null;
        }
        catch (Exception ex)
        {
            Log.Error($"switching an account to a password failed: {CoreError.Describe(ex)}");
            return CoreError.Said(ex, L10n.SigninRefused(), L10n.AccountAlreadySetUp(), L10n.SigninDifferentAccount());
        }
    }

    /// <summary>
    /// The refused add's offer, taken: the sign-in or password just given replaces the credential
    /// of the account already set up, and setup ends as an add does, without adding anything.
    /// </summary>
    /// <param name="choice">What becomes of the calendar and contacts the sign-in does not reach,
    /// when the offer names any (<see cref="ExistingAccountOffer.Unreached"/>); null otherwise.</param>
    internal async Task UpdateExistingAccountAsync(UnreachedUses? choice)
    {
        if (_setupExisting is not { } offer || _app is not { } app || IsSubmitting)
        {
            return;
        }
        IsSubmitting = true;
        try
        {
            await Task.Run(() => app.UseCredentialFrom(offer.AccountId, offer.ConfigToml, choice));
            Log.Info($"an account took the credential a refused add was given: {LogHandle(offer.AccountId)}");
            IsSubmitting = false;
            SetupError = null;
            FinishSetup();
        }
        catch (Exception ex)
        {
            Log.Error($"updating the account already set up failed: {CoreError.Describe(ex)}");
            IsSubmitting = false;
            SetupError = L10n.StatusConnectFailed(CoreError.Said(ex, L10n.SigninRefused(), L10n.AccountAlreadySetUp(), L10n.SigninDifferentAccount()));
        }
    }
}
