// The JMAP "Sign in with your provider" half of the setup form's code-behind, split from
// AccountSetupView.xaml.cs by responsibility (and to keep both under the 500-line limit). This
// file is only the WinUI plumbing, what to show and what stays usable is decided by
// JmapSignInGate, which is WinUI-free and unit-tested (Mailcal.Tests/JmapSignInGateTests.cs).
//
// Two rules shape the plumbing:
//   - the availability pre-flight BLOCKS (it makes network round trips in the core), so it never
//     runs on the UI thread and never per keystroke, each edit restarts a short timer, and only
//     the pause at the end of typing spends a round trip;
//   - the password/API-token field is never taken away by a failure. OAuth is an addition to it.

using System.Threading.Tasks;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    // How long after the last keystroke the availability probe runs. Long enough that typing an
    // address end to end costs one round trip, short enough that the button appears while the user
    // is still looking at the field.
    private static readonly TimeSpan ProbeDebounce = TimeSpan.FromMilliseconds(600);

    private readonly JmapSignInGate _jmapSignIn = new();
    private DispatcherTimer? _jmapProbeTimer;

    // The JMAP email/server changed: re-gate Connect as before, drop any answer that belonged to
    // the old address (so a button can't linger for a server nobody is connecting to), and queue a
    // fresh probe.
    private void OnJmapFieldChanged(object sender, TextChangedEventArgs e)
    {
        // TextChanged can fire while the tree is still being built, before the later fields exist.
        if (JmapServer is null)
        {
            return;
        }
        _jmapSignIn.FieldsChanged(JmapEmail.Text, JmapServer.Text);
        UpdateJmapSignIn();
        UpdateCanConnect();
        ScheduleJmapProbe();
    }

    // Restart the debounce. The tick fires on the UI thread, so the probe it starts resumes there.
    private void ScheduleJmapProbe()
    {
        if (_jmapProbeTimer is null)
        {
            _jmapProbeTimer = new DispatcherTimer { Interval = ProbeDebounce };
            _jmapProbeTimer.Tick += (_, _) =>
            {
                _jmapProbeTimer!.Stop();
                _ = ProbeJmapSignInAsync();
            };
        }
        _jmapProbeTimer.Stop();
        _jmapProbeTimer.Start();
    }

    // Ask the core whether this server offers sign-in at all, off the UI thread. The gate decides
    // whether the round trip is worth making and whether the answer is still the one we asked for.
    private async Task ProbeJmapSignInAsync()
    {
        if (Model is not { } model || JmapChoice.IsChecked != true)
        {
            return;
        }
        if (_jmapSignIn.BeginProbe() is not { } key)
        {
            return;
        }
        var available = await model.JmapOAuthAvailableAsync(JmapEmail.Text, JmapServer.Text);
        _jmapSignIn.Probed(key, available);
        UpdateJmapSignIn();
    }

    // Run the browser sign-in. On success the model has already added and stored the account
    // through the same path a manual JMAP connect uses, and the form closes itself; on failure the
    // inline note goes up and the secret field below is still there.
    private async void OnSignInJmap(object sender, RoutedEventArgs e)
    {
        if (Model is not { } model)
        {
            return;
        }
        _jmapSignIn.SignInStarted();
        UpdateJmapSignIn();
        var outcome = await model.SignInWithJmapAsync(JmapEmail.Text, JmapServer.Text);
        _jmapSignIn.SignInFinished(outcome);
        UpdateJmapSignIn();
        UpdateCanConnect();
    }

    // Reflect the gate onto the panel.
    private void UpdateJmapSignIn()
    {
        // The initial IsChecked on the account-type picker fires while the tree is still being
        // built, so this can run before the JMAP fields exist. JmapPassword is the last control
        // touched below, so it is the one to test.
        if (JmapPassword is null)
        {
            return;
        }
        JmapSignInPanel.Visibility = _jmapSignIn.ShowButton ? Visibility.Visible : Visibility.Collapsed;
        JmapSignInButton.IsEnabled = _jmapSignIn.ButtonEnabled;
        JmapSignInFailedBar.IsOpen = _jmapSignIn.ShowFailure;
        // Only an in-flight sign-in takes the manual secret away; a failure hands it straight back.
        JmapPassword.IsEnabled = _jmapSignIn.ManualEnabled;
        // On a detected card, a server that offers sign-in gets the button alone, see
        // JmapSignInGate.ShowManualSecret for why, and for why a failure reverses it.
        var manual = _jmapSignIn.ShowManualSecret;
        JmapManualPanel.Visibility = manual ? Visibility.Visible : Visibility.Collapsed;
        if (JmapChoice.IsChecked != true)
        {
            return;
        }
        // Connect submits the fields above, so it goes with them: a Connect button under no
        // secret field is a button that can never be pressed. (OnAccountTypeChanged has just set
        // it visible for this tab; this narrows that, and never widens it to another tab.)
        ConnectButton.Visibility = manual ? Visibility.Visible : Visibility.Collapsed;
        // "Your provider supports JMAP. Just add your password or an API token.", true only while
        // there IS a box to add it to. With the sign-in offer up it describes something that is
        // not on screen, and contradicts the offer's own note, so it stands down with the fields.
        DetectNote.Visibility = manual && !string.IsNullOrEmpty(DetectNote.Text)
            ? Visibility.Visible
            : Visibility.Collapsed;
    }

    // Back to square one when the form reopens to add another account.
    private void ResetJmapSignIn()
    {
        _jmapProbeTimer?.Stop();
        _jmapSignIn.Reset();
        UpdateJmapSignIn();
    }
}
