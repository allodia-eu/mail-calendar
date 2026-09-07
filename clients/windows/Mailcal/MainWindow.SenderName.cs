// Raising the "your name" step once an account connects (docs/sending.md). Its own partial so
// MainWindow.xaml.cs stays clear of the 500-line limit.

using System.Threading.Tasks;
using Allodia.Mailcal.Dialogs;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    /// <summary>
    /// Asks what to call the sender of <paramref name="account"/>'s mail, seeded with what its
    /// provider already calls this person, and stores the answer.
    /// </summary>
    /// <remarks>
    /// The request is cleared first, so a second property change cannot raise a second dialog
    /// over the first: WinUI throws on that, and <see cref="DialogHelper"/> would swallow the
    /// show and leave the account unasked with nothing on screen to say so.
    ///
    /// The suggestion is a provider round trip, awaited off the UI thread. A provider that is
    /// slow, unreachable, or short the scope its settings API needs answers empty, which is the
    /// ordinary IMAP case and means <em>ask</em>.
    /// </remarks>
    private async Task AskSenderNameAsync(string account)
    {
        Model.SenderNamePrompt = null;
        var suggestion = await Model.SuggestedSenderNameAsync(account);
        var name = await SenderNameDialog.AskAsync(Content.XamlRoot, suggestion);
        if (name is null)
        {
            // Skipped: the account keeps sending as a bare address, which is a state the app has
            // to be correct in.
            return;
        }
        Model.SetAccountSenderNameChoice(account, name);
    }
}
