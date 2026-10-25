// The question a calendar or address book server found on another site asks, as the calendar and
// contacts screens open it (docs/account-autodetect.md rule 14). The account's page draws the same
// question inline (SettingsDialog.AccountServerChange.cs).

using Allodia.Mailcal.Services;
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
        if (await model.PendingServerChangeAsync(accountId) is not { } pending)
        {
            return;
        }
        // Declared, so the lambda below that replaces it reads it as never null.
        ServerChange change = pending;
        var copy = ServerChangeViews.Copy(change, model.AccountAddress(accountId));
        var content = ServerChangeViews.Panel(copy, titled: false);
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = copy.Title,
            Content = content.Panel,
            PrimaryButtonText = L10n.ServerChangedAllow(),
            CloseButtonText = L10n.ServerChangedNotNow(),
            // Enter and Escape both answer "Not now": allowing a server is never the easy press.
            DefaultButton = ContentDialogButton.Close,
        };
        AutomationProperties.SetAutomationId(dialog, "ServerChangePrompt");
        var busy = false;
        dialog.PrimaryButtonClick += async (_, args) =>
        {
            // Open until the core answers, so a failure is said where the question was asked.
            var deferral = args.GetDeferral();
            busy = true;
            dialog.IsPrimaryButtonEnabled = false;
            content.Say(null);
            content.Busy(true);
            var answer = await model.AcceptServerChangeAsync(accountId, change);
            busy = false;
            content.Busy(false);
            dialog.IsPrimaryButtonEnabled = true;
            switch (answer)
            {
                case ServerChangeAnswer.Moved:
                    break;
                case ServerChangeAnswer.Replaced replaced:
                    // A lookup replaced the question while it was shown, and nothing moved: ask
                    // the new one in its place.
                    args.Cancel = true;
                    change = replaced.Change;
                    copy = ServerChangeViews.Copy(change, model.AccountAddress(accountId));
                    content = ServerChangeViews.Panel(copy, titled: false);
                    dialog.Title = copy.Title;
                    dialog.Content = content.Panel;
                    break;
                default:
                    args.Cancel = true;
                    content.Say(L10n.ServerChangedFailed());
                    break;
            }
            deferral.Complete();
        };
        // Closing would not stop an allow already under way, so "Not now" waits for its answer.
        dialog.Closing += (_, args) =>
        {
            if (busy)
            {
                args.Cancel = true;
            }
        };
        await DialogHelper.ShowAsync(dialog);
    }
}
