// An account's page, the question a switch to the provider asks before the browser opens when the
// new sign-in will not reach the account's calendar or contacts (docs/accounts.md rule 18). It is
// drawn in the Sign-in section, under the buttons, because Settings is itself a ContentDialog. The
// question waiting is kept on the dialog rather than in the panel, so a redraw while it is open
// draws it again, and closing Settings answers it as Cancel.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private sealed record UnreachedAsking(
        string AccountId, IReadOnlyList<AccountCapability> Unreached, TaskCompletionSource<UnreachedAnswer> Answer);

    private UnreachedAsking? _unreachedAsking;

    // The question as drawn last, which an answer takes away.
    private StackPanel? _unreachedDrawn;

    // The Sign-in section drawn last, and whose: where a question asked after the drawing goes.
    private StackPanel? _signInControls;
    private string? _signInDrawnFor;

    private bool _unreachedClosedWired;

    private static AccountNotice SignInWaiting(AccountEntry entry) =>
        new(InfoBarSeverity.Informational, string.Empty, L10n.SettingsAccountSigninWaiting(entry.Address));

    /// <summary>Asks what becomes of <paramref name="unreached"/>; answered by the panel, or as
    /// Cancel when Settings closes first.</summary>
    private Task<UnreachedAnswer> AskUnreachedAsync(AccountEntry entry, IReadOnlyList<AccountCapability> unreached)
    {
        var answer = new TaskCompletionSource<UnreachedAnswer>(TaskCreationOptions.RunContinuationsAsynchronously);
        if (!_unreachedClosedWired)
        {
            _unreachedClosedWired = true;
            Closed += (_, _) => _unreachedAsking?.Answer.TrySetResult(UnreachedAnswer.Cancel);
        }
        _unreachedAsking = new UnreachedAsking(entry.Id, unreached, answer);
        // Nothing is waiting on the browser yet: the question comes first.
        SayOnAccounts(null);
        if (_signInDrawnFor == entry.Id && _signInControls is { } controls)
        {
            DrawUnreachedQuestion(controls, entry.Id);
        }
        return answer.Task;
    }

    /// <summary>Draws the question waiting for <paramref name="accountId"/>, if there is one, at
    /// the end of its Sign-in section.</summary>
    private void DrawUnreachedQuestion(StackPanel controls, string accountId)
    {
        _signInControls = controls;
        _signInDrawnFor = accountId;
        if (_unreachedAsking is not { } asking || asking.AccountId != accountId || asking.Answer.Task.IsCompleted)
        {
            return;
        }
        var question = UnreachedUsesViews.Question(asking.Unreached, _brushes.Of, answer =>
        {
            RemoveUnreachedQuestion();
            if (answer != UnreachedAnswer.Cancel
                && _model.AccountsSnapshot().Accounts.FirstOrDefault(entry => entry.Id == accountId) is { } entry)
            {
                SayOnAccounts(SignInWaiting(entry));
            }
            asking.Answer.TrySetResult(answer);
        });
        _unreachedDrawn = question;
        controls.Children.Add(question);
    }

    /// <summary>The sign-in ended, however it did: any question still open goes with it.</summary>
    private void EndUnreachedQuestion()
    {
        _unreachedAsking?.Answer.TrySetResult(UnreachedAnswer.Cancel);
        _unreachedAsking = null;
        RemoveUnreachedQuestion();
    }

    private void RemoveUnreachedQuestion()
    {
        if (_unreachedDrawn is { Parent: Panel parent } drawn)
        {
            parent.Children.Remove(drawn);
        }
        _unreachedDrawn = null;
    }
}
