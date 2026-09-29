// Moving between the setup form's two steps, and leaving it. The address step comes first; the
// detected or manual form is the second, and it always has a way back to the first
// (docs/account-autodetect.md, rule 12). Its own partial so the code-behind stays clear of the
// 500-line limit.

using Microsoft.UI.Xaml;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    /// <summary>
    /// Whether the form sits in a dialog, which brings its own padding, or fills the window on a
    /// first run.
    /// </summary>
    internal void UseDialogLayout(bool inDialog) =>
        FormRoot.Margin = inDialog ? new Thickness(0) : new Thickness(24);

    /// <summary>
    /// What dismissing the dialog means, which is what the form's own Cancel means: an outstanding
    /// browser sign-in is abandoned and the form stays; otherwise the add is abandoned. A bounded
    /// connect in flight is left to finish, so the dialog stays for its answer.
    /// </summary>
    /// <returns>Whether the dialog may close.</returns>
    internal bool Dismiss()
    {
        if (Model is null)
        {
            return true;
        }
        if (Model.IsSigningIn)
        {
            CancelSignIns();
            return false;
        }
        if (Model.IsSubmitting)
        {
            return false;
        }
        Model.CancelAddAccount();
        return true;
    }

    // Only one browser sign-in runs at a time; cancelling the others is a safe no-op.
    private void CancelSignIns()
    {
        Model?.CancelMicrosoftSignIn();
        Model?.CancelGoogleSignIn();
        Model?.CancelJmapSignIn();
    }

    // Back to the address, which keeps what was typed there. Everything the abandoned route
    // filled in goes with it, including a certificate it was refused or accepted: a different
    // address can reach a different server, and nothing is accepted that was not shown.
    private void OnBackToDetect(object sender, RoutedEventArgs e)
    {
        Model?.ClearSetupAttempt();
        ShowDetectStep();
        DetectEmail.Focus(FocusState.Programmatic);
    }

    // The address step, with the second step's fields cleared behind it.
    private void ShowDetectStep()
    {
        ClearManualFields();
        _needsApproval = false;
        ApprovalPanel.Visibility = Visibility.Collapsed;
        DetectNote.Visibility = Visibility.Collapsed;
        SetupPanel.Visibility = Visibility.Collapsed;
        DetectPanel.Visibility = Visibility.Visible;
        ContinueButton.IsEnabled = !string.IsNullOrWhiteSpace(DetectEmail.Text);
    }
}
