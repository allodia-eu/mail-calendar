// The mail-account half of the setup form's code-behind: asking the server what it accepts, and
// the "Sign in with your provider" button where it accepts one. Split from
// AccountSetupView.xaml.cs by responsibility (and to keep both under the 500-line limit).
//
// This file is only the WinUI plumbing. What to show is decided by ImapSignInGate, which is
// WinUI-free and unit-tested (Mailcal.Tests/ImapSignInGateTests.cs).
//
// Three rules shape the plumbing, the first two the same ones the JMAP half keeps:
//   - the pre-flight BLOCKS (it dials the mail server), so it never runs on the UI thread and
//     never per keystroke: each edit restarts a short timer, and only the pause at the end of
//     typing spends a dial;
//   - no failure ever leaves somebody with no way in, so a failed sign-in hands the password
//     field straight back, whatever the server said about passwords;
//   - a deadline races the pre-flight (docs/mail-oauth.md rule 8), so a core that stops
//     answering still leaves a password field to type into.

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
        // The port and security of each server as Connect submits them: detected, or chosen by
        // the person on the manual form.
        _imap.Dial(ImapHost.Text.Trim()),
        string.IsNullOrWhiteSpace(SmtpHost.Text) ? null : _smtp.Dial(SmtpHost.Text.Trim()),
        string.IsNullOrWhiteSpace(CaldavUrl.Text) ? null : CaldavUrl.Text.Trim(),
        _imap.Security,
        _smtp.Security,
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
        UpdateImapSignIn();
        var asked = model.ImapAuthOptionsAsync(ImapRequest());
        if (await Task.WhenAny(asked, Task.Delay(ImapSignInGate.Deadline)) != asked
            && _imapSignIn.DeadlinePassed(key))
        {
            UpdateImapSignIn();
            UpdateCanConnect();
        }
        // After a deadline this is dropped by the gate: only the first answer counts.
        _imapSignIn.Answered(key, await asked);
        UpdateImapSignIn();
        UpdateCanConnect();
    }

    // "Use a password instead": the field, and Connect with it, for somebody who would rather not
    // sign in through the browser. The sign-in stays on screen as an ordinary button.
    private void OnUseImapPassword(object sender, RoutedEventArgs e)
    {
        _imapSignIn.RevealPassword();
        UpdateImapSignIn();
        UpdateCanConnect();
        Password.Focus(FocusState.Programmatic);
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
        if (ImapSignInPanel is null || ImapPasswordPanel is null)
        {
            return;
        }
        var checking = _imapSignIn.Checking;
        ImapCheckingPanel.Visibility = checking ? Visibility.Visible : Visibility.Collapsed;
        ImapCheckingRing.IsActive = checking;
        ImapSignInPanel.Visibility = _imapSignIn.ShowButton ? Visibility.Visible : Visibility.Collapsed;
        ImapSignInButton.IsEnabled = _imapSignIn.ButtonEnabled;
        // Primary until a password is on screen beside it; then Connect leads (rule 2). Assigned
        // only on a change, because a new style re-templates the button under a focused pointer.
        var style = (Style) Application.Current.Resources[
            _imapSignIn.SignInLeads ? "AccentButtonStyle" : "DefaultButtonStyle"];
        if (!ReferenceEquals(ImapSignInButton.Style, style))
        {
            ImapSignInButton.Style = style;
        }
        ImapUsePasswordButton.Visibility =
            _imapSignIn.ShowPasswordInstead ? Visibility.Visible : Visibility.Collapsed;
        ImapUsePasswordButton.IsEnabled = _imapSignIn.PasswordEnabled;
        ImapSignInFailedBar.IsOpen = _imapSignIn.ShowFailure;
        ImapRegistrationNeededNote.Visibility =
            _imapSignIn.ShowRegistrationNeeded ? Visibility.Visible : Visibility.Collapsed;

        var password = _imapSignIn.ShowPassword;
        ImapPasswordPanel.Visibility = password ? Visibility.Visible : Visibility.Collapsed;
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
