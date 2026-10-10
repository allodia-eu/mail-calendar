// The form's actions: Connect for the password routes, and the browser sign-ins for Microsoft and
// Google with the Early Access gate in front of Google's. Each takes what the form says the account
// is used for. Split from AccountSetupView.xaml.cs to keep that file under the 500-line limit.

using Allodia.Mailcal;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    private void OnConnect(object sender, RoutedEventArgs e)
    {
        var accepted = CertificateCheck.IsChecked == true ? RefusedCertificate() : null;
        if (JmapChoice.IsChecked == true)
        {
            Model?.SubmitJmapSetup(JmapEmail.Text, JmapServer.Text, JmapPassword.Password);
            return;
        }
        if (DavChoice.IsChecked == true)
        {
            ConnectDav(accepted);
            return;
        }
        // The found card's choices decide the uses and the calendar and address-book servers;
        // the manual form's fields decide them otherwise.
        if (FlagMissingServer())
        {
            return;
        }
        var chosen = _uses.Choices.Count > 0 ? ChosenUses() : null;
        Model?.SubmitSetup(
            _imap.Dial(ImapHost.Text),
            Username.Text,
            Password.Password,
            _smtp.Dial(SmtpHost.Text),
            chosen?.CaldavUrl ?? CaldavUrl.Text,
            _imap.Security,
            _smtp.Security,
            accepted,
            chosen?.CarddavUrl ?? CarddavUrl.Text,
            chosen?.Uses);
    }

    // Pass the address the user typed in the email-first step (empty on a purely manual pick), so
    // Microsoft targets that account rather than a different one already signed in in the browser.
    private void OnSignInMicrosoft(object sender, RoutedEventArgs e) =>
        Model?.SignInWithMicrosoft(DetectEmail.Text, ProviderUses(AccountKind.Microsoft));

    // Pass the typed address as the login hint, same as Microsoft. The Early Access checkbox has
    // already gated this button to enabled, so no extra check is needed here.
    private void OnSignInGoogle(object sender, RoutedEventArgs e) =>
        Model?.SignInWithGoogle(DetectEmail.Text, ProviderUses(AccountKind.Google));

    // The Early Access checkbox gates the Google sign-in button: the user must confirm they've
    // signed up (Gmail is allow-listed while Google reviews the app) before we can start the flow.
    private void OnGoogleEarlyAccessChanged(object sender, RoutedEventArgs e) => UpdateGoogleSignInEnabled();

    // Open the Early Access sign-up page in the default browser.
    private async void OnOpenGoogleEarlyAccess(object sender, RoutedEventArgs e) =>
        await Windows.System.Launcher.LaunchUriAsync(new System.Uri(L10n.SetupGoogleEarlyAccessUrl()));

    // The Google sign-in button is enabled only once the Early Access box is checked AND nothing is
    // already submitting (so it can't fire twice or while a connect is in flight).
    private void UpdateGoogleSignInEnabled()
    {
        if (GoogleButton is null)
        {
            return;
        }
        GoogleButton.IsEnabled = Model is { IsSubmitting: false } && GoogleEarlyAccessCheck.IsChecked == true;
    }
}
