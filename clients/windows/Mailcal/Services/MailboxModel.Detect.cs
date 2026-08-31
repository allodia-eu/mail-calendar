// The email-first detection half of MailboxModel (split out to keep each file under the 500-line
// limit): run the shared core's account-settings lookup off the UI thread, with the device's own
// DNS answering the MX fallback via WindowsMxResolver. The Windows counterpart of Android's
// detectAccount and macOS's detectSetup.

using System.Threading.Tasks;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Detects a provider's settings from just the email address, off the UI thread (the core
    /// call blocks up to ~10 s). Returns a manual fallback if the app somehow isn't up (the setup
    /// flow only shows when it is).
    /// </summary>
    internal async Task<SetupRecommendation> DetectAsync(string email)
    {
        if (_app is null)
        {
            return new SetupRecommendation.Manual(MissReason.NetworkError);
        }
        var app = _app;
        return await Task.Run(() => app.DetectAccountSettings(email, new WindowsMxResolver())).ConfigureAwait(false);
    }
}
