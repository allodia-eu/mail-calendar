// The Allodia account's half of the expired sign-in banners: the service refused this device's
// grant, so the account that keeps the person's mail accounts in step across devices has stopped
// doing so. Its own partial so MailboxModel.Connectivity.cs stays clear of the 500-line limit.

using System.Threading.Tasks;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    // The address whose Allodia sign-in the service refused, or null. The core clears it on a
    // sign-in, a sign-out or a later call the service accepts, and signals Connectivity either way.
    private string? _allodiaSignInEnded;
    // What the banner says after the sign-in it started did not complete, or null.
    private string? _allodiaBannerFailure;

    private void UpdateAllodiaSignInEnded(ConnectivitySnapshot connectivity)
    {
        // A failure answers the attempt on this address, so a different address, or none, drops it.
        if (connectivity.AllodiaSignInEnded != _allodiaSignInEnded)
        {
            _allodiaBannerFailure = null;
        }
        _allodiaSignInEnded = connectivity.AllodiaSignInEnded;
        Raise(nameof(HasAllodiaSignInEnded));
        Raise(nameof(AllodiaSignInEndedText));
    }

    /// <summary>Whether the service refused this device's Allodia sign-in, which drives its banner.
    /// Shown regardless of the offline state, like the mail accounts' prompts.</summary>
    public bool HasAllodiaSignInEnded => AllodiaSignInEndedBanner.Shown(_allodiaSignInEnded);

    /// <summary>The banner's message, naming the address, and the last attempt's failure after it.</summary>
    public string AllodiaSignInEndedText => _allodiaSignInEnded is { } email
        ? AllodiaSignInEndedBanner.Message(L10n.AllodiaSigninEndedPrompt(email), _allodiaBannerFailure)
        : string.Empty;

    /// <summary>The banner's button: the ordinary Allodia sign-in, run from where the person is.
    /// A no-op while a sign-in is already waiting on the browser, whose redirect it would
    /// otherwise orphan.</summary>
    public void SignInToAllodiaFromBanner()
    {
        if (_allodiaSignIn is not null)
        {
            return;
        }
        _ = SignInToAllodiaFromBannerAsync();
    }

    private async Task SignInToAllodiaFromBannerAsync()
    {
        SetAllodiaBannerFailure(null);
        var failure = await SignInToAllodiaAsync();
        SetAllodiaBannerFailure(AllodiaSignInEndedBanner.AfterAttempt(
            failure is not null, failure?.Stale == true, L10n.SigninStale(), L10n.SigninExpiredFailed()));
        // What the prompt promised: the accounts in step again, now rather than at the next
        // launch. Asked of the grant, because a sign-in the person gave up on also returns null.
        if (failure is null && AllodiaGrantHealth == AllodiaGrantHealth.Ok)
        {
            _ = SyncAllodiaAccountsAsync();
        }
    }

    private void SetAllodiaBannerFailure(string? failure)
    {
        _allodiaBannerFailure = failure;
        Raise(nameof(AllodiaSignInEndedText));
    }
}
