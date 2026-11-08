// The Allodia account's refused sign-in: the service refused this device's grant, so the account
// that keeps the person's mail accounts in step across devices has stopped doing so. The window
// says so through its notice (MailboxModel.Notices.cs); Settings reads it from here, which is read
// from that notice too.

using System.Threading.Tasks;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    // Whether the AllodiaSignInEnded notice is in force. The core withdraws it on a sign-in, a
    // sign-out or a later call the service accepts, and signals Notices either way.
    private bool _allodiaSignInEnded;

    private void UpdateAllodiaSignInEnded(bool ended)
    {
        _allodiaSignInEnded = ended;
        Raise(nameof(HasAllodiaSignInEnded));
    }

    /// <summary>Whether the service refused this device's Allodia sign-in. A grant that is merely
    /// narrower than this build wants raises no such notice.</summary>
    public bool HasAllodiaSignInEnded => _allodiaSignInEnded;

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
