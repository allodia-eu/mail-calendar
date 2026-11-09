// The question a calendar or address book server found on another site asks, as the calendar and
// contacts screens open it (docs/account-autodetect.md rule 14). Its buttons are the question's
// own, drawn from the core's record; Escape is the dialog's, and waits while Allow runs. The account's page draws the same
// question inline (SettingsDialog.AccountServerChange.cs). Both draw the core's record, read again
// whenever the accounts change: an allow starting or ending, or a lookup replacing the question.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

internal static class ServerChangeDialog
{
    /// <summary>Asks about <paramref name="accountId"/>'s pending change; shows nothing when none
    /// is pending, such as one allowed meanwhile.</summary>
    internal static async Task AskAsync(XamlRoot root, MailboxModel model, string accountId)
    {
        if (model.ServerChangeQuestion(accountId) is not { } first)
        {
            return;
        }
        var drawn = first;
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = ServerChangeViews.Copy(first.Change, AccountNameText.Of(first.Account)).Title,
        };
        AutomationProperties.SetAutomationId(dialog, "ServerChangePrompt");
        void Draw(AccountServerChange question)
        {
            drawn = question;
            dialog.Title = ServerChangeViews.Copy(question.Change, AccountNameText.Of(question.Account)).Title;
            var (panel, notNow) = ServerChangeViews.Question(
                question, titled: false,
                () => model.AllowServerChange(accountId, question.Change), dialog.Hide);
            // Return and Escape both answer "Not now": allowing a server is never the easy press.
            // The focus goes to it each time the question is drawn, since the Allow just pressed
            // is gone with the content it replaced.
            panel.Loaded += (_, _) => notNow.Focus(FocusState.Programmatic);
            dialog.Content = panel;
        }
        void Redraw()
        {
            switch (model.ServerChangeQuestion(accountId))
            {
                // Settled, here or elsewhere: nothing waits any more.
                case null:
                    dialog.Hide();
                    break;
                case var question when question != drawn:
                    Draw(question);
                    break;
            }
        }
        Draw(first);
        model.AccountsChanged += Redraw;
        // Closing would not stop an allow already under way, so "Not now" waits for its answer.
        dialog.Closing += (_, args) =>
        {
            if (model.ServerChangeQuestion(accountId) is { Allow.Running: true })
            {
                args.Cancel = true;
            }
        };
        dialog.Closed += (_, _) => model.AccountsChanged -= Redraw;
        await DialogHelper.ShowAsync(dialog);
    }
}
