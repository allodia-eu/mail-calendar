// The Allodia account's refused sign-in: the service refused this device's grant, so the account
// that keeps the person's mail accounts in step across devices has stopped doing so. The window
// says so through its notice (MailboxModel.Notices.cs); Settings reads it from here.

using System.Threading.Tasks;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    // The address whose Allodia sign-in the service refused, or null. The core clears it on a
    // sign-in, a sign-out or a later call the service accepts, and signals Connectivity either way.
    private string? _allodiaSignInEnded;

    private void UpdateAllodiaSignInEnded(ConnectivitySnapshot connectivity)
    {
        _allodiaSignInEnded = connectivity.AllodiaSignInEnded;
        Raise(nameof(HasAllodiaSignInEnded));
    }

    /// <summary>Whether the service refused this device's Allodia sign-in. A grant that is merely
    /// narrower than this build wants never names an address.</summary>
    public bool HasAllodiaSignInEnded => !string.IsNullOrEmpty(_allodiaSignInEnded);

    /// <summary>
    /// The notice's sign-in: the ordinary Allodia sign-in, run from where the person is.
    /// </summary>
    /// <returns>Whether it failed. A sign-in the person gave up on, or whose browser never came
    /// back, did not, and neither did one already waiting on the browser, whose redirect a second
    /// would orphan.</returns>
    private async Task<bool> SignInToAllodiaFromNoticeAsync()
    {
        if (_allodiaSignIn is not null)
        {
            return false;
        }
        var failure = await SignInToAllodiaAsync();
        // What the notice promised: the accounts in step again, now rather than at the next
        // launch. Asked of the grant, because a sign-in the person gave up on also returns null.
        if (failure is null && AllodiaGrantHealth == AllodiaGrantHealth.Ok)
        {
            _ = SyncAllodiaAccountsAsync();
        }
        return NoticeRules.RemedyFailed(signedIn: false, failure?.Said, failure?.Unanswered == true);
    }
}
