// The link step's offer of the servers detection found beside a JMAP server (docs/onboarding.md,
// "After the connect: linking another account"): what detection found is remembered for the JMAP
// account added from that card, and taking the offer runs setup again on dav_setup_beside's card,
// coming back to the link step as "Add another account" does. The rules are SetupBeside's.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private readonly SetupBeside _setupBeside = new();

    /// <summary>The card the form opens on instead of the address step: the servers found beside a
    /// JMAP server, set up as an account of their own. Taken by the form once drawn.</summary>
    internal DetectedSetup? SetupStartDetected { get; private set; }

    /// <summary>The form takes <see cref="SetupStartDetected"/>.</summary>
    internal DetectedSetup? TakeSetupStartDetected()
    {
        var detected = SetupStartDetected;
        SetupStartDetected = null;
        return detected;
    }

    /// <summary>Detection answered, or a card was opened from an offer: what it found beside a
    /// JMAP server is kept for the account set up from it.</summary>
    internal void SetupDetected(DetectedSetup setup) => _setupBeside.Detected(setup);

    /// <summary>
    /// The link step's offer taken: setup again on the card of the servers found beside its JMAP
    /// server, for only what the session lacks, coming back to the step as "Add another account"
    /// does. The offer is used up. Nothing is opened when the session has meanwhile come to hold
    /// what they would fill; the step loses the offer instead.
    /// </summary>
    internal void SetupBesideAccount()
    {
        if (_savingLinks || _app is not { } app || _setupFlow.Links is not { } step
            || _setupBeside.Take(step.Account) is not { } offer)
        {
            return;
        }
        var found = offer.Found;
        if (app.DavSetupBeside(offer.Account, found.Email, found.CaldavUrl, found.CarddavUrl, found.Nextcloud) is not { } setup)
        {
            if (_setupFlow.Refresh(LinkStepFor(step.Account)))
            {
                SetupLinksChanged?.Invoke();
            }
            return;
        }
        _setupFlow.AddLinked();
        SetupError = null;
        SetupStartEmail = found.Email;
        SetupStartOffer = null;
        SetupStartDetected = setup;
        ClearSetupAttempt();
        RaiseSetupFlow();
    }
}
