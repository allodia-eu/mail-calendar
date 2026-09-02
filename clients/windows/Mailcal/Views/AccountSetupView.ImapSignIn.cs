// The mail-account half of the setup form's code-behind: asking the server what it accepts, and
// the "Sign in with your provider" button where it accepts one. Split from
// AccountSetupView.xaml.cs by responsibility (and to keep both under the 500-line limit).
//
// This file is only the WinUI plumbing. What to show is decided by ImapSignInGate, which is
// WinUI-free and unit-tested (Mailcal.Tests/ImapSignInGateTests.cs).
//
// Two rules shape the plumbing, the same two the JMAP half keeps:
//   - the pre-flight BLOCKS (it dials the mail server), so it never runs on the UI thread and
//     never per keystroke: each edit restarts a short timer, and only the pause at the end of
//     typing spends a dial;
//   - no failure ever leaves somebody with no way in, so a failed sign-in hands the password
//     field straight back, whatever the server said about passwords.

using System.Threading.Tasks;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    private readonly ImapSignInGate _imapSignIn = new();
    private DispatcherTimer? _imapProbeTimer;

    // The account the pre-flight and the sign-in both describe. One builder used by both, so the
    // two cannot come to different conclusions about the same account: a pre-flight that probed a
    // different server from the one the sign-in registers against would offer a button that fails
    // at the provider.
    private ImapLoginRequest ImapRequest() => new(
        Username.Text.Trim(),
        ImapHost.Text.Trim(),
        string.IsNullOrWhiteSpace(SmtpHost.Text) ? null : SmtpHost.Text.Trim(),
        string.IsNullOrWhiteSpace(CaldavUrl.Text) ? null : CaldavUrl.Text.Trim(),
        // The detected route carries the security it found; the manual form is implicit-TLS only
        // (docs/account-autodetect.md → Known gaps).
        _detectedImapSecurity,
        _detectedSmtpSecurity,
        _detectedOauthIssuer);

    // Restart the debounce. The tick fires on the UI thread, so the ask it starts resumes there.
    private void ScheduleImapProbe()
    {
        if (_imapProbeTimer is null)
        {
            _imapProbeTimer = new DispatcherTimer { Interval = ProbeDebounce };
            _imapProbeTimer.Tick += (_, _) =>
            {
                _imapProbeTimer!.Stop();
                _ = AskImapServerAsync();
            };
        }
        _imapProbeTimer.Stop();
        _imapProbeTimer.Start();
    }

    // Ask the mail server what it accepts, off the UI thread. The gate decides whether the dial is
    // worth making and whether the answer is still the one we asked for.
    private async Task AskImapServerAsync()
    {
        if (Model is not { } model || ImapChoice.IsChecked != true)
        {
            return;
        }
        if (_imapSignIn.BeginAsking() is not { } key)
        {
            return;
        }
        var answer = await model.ImapAuthOptionsAsync(ImapRequest());
        _imapSignIn.Answered(key, answer);
        UpdateImapSignIn();
        UpdateCanConnect();
    }

    // Run the browser sign-in. On success the model has already added and stored the account
    // through the same path a password connect uses, and the form closes itself; on failure the
    // inline note goes up and the password field below is back.
    private async void OnSignInImap(object sender, RoutedEventArgs e)
    {
        if (Model is not { } model)
        {
            return;
        }
        _imapSignIn.SignInStarted();
        UpdateImapSignIn();
        var outcome = await model.SignInWithImapAsync(ImapRequest());
        _imapSignIn.SignInFinished(outcome);
        UpdateImapSignIn();
        UpdateCanConnect();
    }

    // Reflect the gate onto the panel.
    private void UpdateImapSignIn()
    {
        // The initial IsChecked on the account-type picker fires while the tree is still being
        // built, so this can run before the fields exist.
        if (ImapSignInPanel is null || Password is null)
        {
            return;
        }
        ImapSignInPanel.Visibility = _imapSignIn.ShowButton ? Visibility.Visible : Visibility.Collapsed;
        ImapSignInButton.IsEnabled = _imapSignIn.ButtonEnabled;
        ImapSignInFailedBar.IsOpen = _imapSignIn.ShowFailure;
        ImapRegistrationNeededNote.Visibility =
            _imapSignIn.ShowRegistrationNeeded ? Visibility.Visible : Visibility.Collapsed;

        var password = _imapSignIn.ShowPassword;
        Password.Visibility = password ? Visibility.Visible : Visibility.Collapsed;
        Password.IsEnabled = _imapSignIn.PasswordEnabled;
        if (ImapChoice.IsChecked != true)
        {
            return;
        }
        // Connect submits that field, so it goes with it: a Connect button under no password field
        // is a button that can never be pressed. (OnAccountTypeChanged has just set it visible for
        // this tab; this narrows that, and never widens it to another tab.)
        ConnectButton.Visibility = password ? Visibility.Visible : Visibility.Collapsed;
    }

    // Back to square one when the form reopens to add another account.
    private void ResetImapSignIn()
    {
        _imapProbeTimer?.Stop();
        _imapSignIn.Reset();
        UpdateImapSignIn();
    }
}
