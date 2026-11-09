// The credential an add refused as a duplicate, put on the account already there
// (docs/accounts.md rule 17). The core connects with the new credential, stores it and replaces
// the account itself; nothing here calls add_account or writes the Credential Manager. Signing an
// account in through its browser is the core's (MailboxModel.BrowserSignIn.cs).

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
        // A Nextcloud grant is the core's from here: it goes on the account, or is withdrawn below,
        // so setup no longer holds it for another try.
        if (offer.NextcloudGrant is not null)
        {
            _ = _setupNextcloud.Release();
        }
        try
        {
            if (offer.NextcloudGrant is { } grant)
            {
                await Task.Run(() => app.CompleteAccountNextcloudSignIn(offer.AccountId, grant));
            }
            else
            {
                await Task.Run(() => app.UseCredentialFrom(offer.AccountId, offer.ConfigToml, choice));
            }
            Log.Info($"an account took the credential a refused add was given: {LogHandle(offer.AccountId)}");
            IsSubmitting = false;
            SetupError = null;
            FinishSetup();
        }
        catch (Exception ex)
        {
            // An unused grant leaves the server's list of devices; one for somebody else the core
            // has withdrawn already.
            if (offer.NextcloudGrant is { } unused && ex is not MailcalException.DifferentAccount)
            {
                AbandonNextcloudGrant(unused);
            }
            Log.Error($"updating the account already set up failed: {CoreError.Describe(ex)}");
            IsSubmitting = false;
            SetupError = CoreErrorCopy.SaidAsConnect(ex);
        }
    }
}
