// Adding another account: the setup form in a dialog over the running app, as macOS puts it in a
// sheet. A first run keeps the form in the window, because there is nothing yet for a dialog to
// sit over. There is one form, and it moves: out of the window's host into the dialog, and back
// when the dialog closes, so the two places cannot drift apart.

using System.Threading.Tasks;
using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    private ContentDialog? _addAccountDialog;
    // Set once the dialog is on its way out, so a close the host started is not read as the
    // person dismissing it, and a dismissal does not ask for a second close.
    private bool _addAccountClosing;

    private void WireAddAccountDialog() =>
        Model.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName != nameof(MailboxModel.AddingAccount))
            {
                return;
            }
            if (Model.AddingAccount)
            {
                _ = ShowAddAccountDialogAsync();
            }
            else if (_addAccountDialog is { } dialog && !_addAccountClosing)
            {
                // Connected, or the form's own Cancel.
                _addAccountClosing = true;
                dialog.Hide();
            }
        };

    private async Task ShowAddAccountDialogAsync()
    {
        if (_addAccountDialog is not null)
        {
            return;
        }
        // Settings opens the form from one of its offers and hides itself first; its close has
        // not finished yet, and a second dialog while it is open would be dropped.
        await DialogHelper.WhenIdleAsync();
        if (!Model.AddingAccount || _addAccountDialog is not null)
        {
            return;
        }
        SetupHost.Child = null;
        SetupView.UseDialogLayout(inDialog: true);
        var dialog = new ContentDialog { XamlRoot = Content.XamlRoot, Content = SetupView };
        // The form titles itself, and its title changes with the step, so the dialog carries no
        // title of its own; a screen reader still needs one to announce.
        AutomationProperties.SetName(dialog, L10n.SetupDetectTitle());
        AutomationProperties.SetAutomationId(dialog, "AddAccountDialog");
        // Wide enough for the server, port and security row the form lays out at 480.
        dialog.Resources["ContentDialogMaxWidth"] = 560.0;
        dialog.Closing += OnAddAccountDialogClosing;
        _addAccountDialog = dialog;
        _addAccountClosing = false;
        try
        {
            await DialogHelper.ShowAsync(dialog);
        }
        finally
        {
            dialog.Content = null;
            SetupView.UseDialogLayout(inDialog: false);
            SetupHost.Child = SetupView;
            _addAccountDialog = null;
            _addAccountClosing = false;
            // A show that was dropped leaves an add nobody can see; take it back.
            if (Model.AddingAccount)
            {
                Model.CancelAddAccount();
            }
        }
    }

    // Escape, which a ContentDialog answers by closing. It means what the form's Cancel means.
    private void OnAddAccountDialogClosing(ContentDialog sender, ContentDialogClosingEventArgs args)
    {
        if (_addAccountClosing)
        {
            return;
        }
        _addAccountClosing = true;
        if (!SetupView.Dismiss())
        {
            _addAccountClosing = false;
            args.Cancel = true;
        }
    }
}
