// An account's page, the question a switch to the provider asks before the browser opens when the
// new sign-in will not reach the account's calendar or contacts (docs/accounts.md rule 18). The
// core holds the sign-in at the question (BrowserSignInStage.Asking) until it is answered; this
// asks it once for each attempt, over the whole of Settings, never at the end of the Sign-in
// section, which is usually below the fold. UnreachedUsesViews.Ask says why it is not a
// ContentDialog. Closing Settings answers it as Cancel.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The question open, and the attempt it was asked for, which is not asked twice.
    private UnreachedPrompt? _unreachedAsking;
    private ulong? _unreachedAttempt;

    private bool _unreachedClosedWired;

    /// <summary>Asks the question the open page's sign-in stands at, or takes away one it has
    /// moved past, unanswered.</summary>
    private void AskUnreached(AccountPage? page)
    {
        var browser = page?.SignIn?.Browser;
        if (BrowserSignInRules.Asking(browser) is not { } unreached || browser is null)
        {
            EndUnreachedQuestion();
            return;
        }
        if (_unreachedAttempt == browser.Attempt)
        {
            return;
        }
        if (!_unreachedClosedWired)
        {
            _unreachedClosedWired = true;
            Closed += (_, _) => UnreachedOnClose();
        }
        EndUnreachedQuestion();
        _unreachedAttempt = browser.Attempt;
        // Over the dialog's own root, which no redraw of the page replaces.
        var prompt = UnreachedUsesViews.Ask((FrameworkElement)Content, unreached, _brushes.Of);
        _unreachedAsking = prompt;
        _ = AnswerUnreachedAsync(browser, prompt);
    }

    private async Task AnswerUnreachedAsync(BrowserSignIn browser, UnreachedPrompt prompt)
    {
        var answer = await prompt.Answer;
        // A question taken away because the sign-in moved past it was not answered.
        if (!ReferenceEquals(_unreachedAsking, prompt))
        {
            return;
        }
        _unreachedAsking = null;
        _model.AnswerUnreached(browser, UnreachedUsesRules.Choice(answer));
    }

    /// <summary>Takes the question away unanswered: the sign-in moved past it.</summary>
    private void EndUnreachedQuestion()
    {
        var open = _unreachedAsking;
        _unreachedAsking = null;
        _unreachedAttempt = null;
        open?.Dismiss();
    }

    /// <summary>Settings closed: a sign-in still at the question is answered Cancel, since
    /// nothing else would answer it.</summary>
    private void UnreachedOnClose()
    {
        EndUnreachedQuestion();
        if (_model.BrowserSignInNow() is { Stage: BrowserSignInStage.Asking } asking)
        {
            _model.AnswerUnreached(asking, null);
        }
    }
}
