// An account's page, the question a switch to the provider asks before the browser opens when the
// new sign-in will not reach the account's calendar or contacts (docs/accounts.md rule 18). It is
// asked over the whole of Settings, never at the end of the Sign-in section, which is usually below
// the fold; UnreachedUsesViews.Ask says why it is not a ContentDialog. Closing Settings answers it
// as Cancel.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private UnreachedPrompt? _unreachedAsking;

    private bool _unreachedClosedWired;

    /// <summary>Asks what becomes of <paramref name="unreached"/>; answered as Cancel when
    /// Settings closes first.</summary>
    private Task<UnreachedAnswer> AskUnreachedAsync(IReadOnlyList<AccountCapability> unreached)
    {
        if (!_unreachedClosedWired)
        {
            _unreachedClosedWired = true;
            Closed += (_, _) => EndUnreachedQuestion();
        }
        EndUnreachedQuestion();
        // Over the dialog's own root, which no redraw of the page replaces.
        _unreachedAsking = UnreachedUsesViews.Ask((FrameworkElement)Content, unreached, _brushes.Of);
        return _unreachedAsking.Answer;
    }

    /// <summary>The sign-in ended, however it did: any question still open goes with it.</summary>
    private void EndUnreachedQuestion()
    {
        _unreachedAsking?.Dismiss();
        _unreachedAsking = null;
    }
}
