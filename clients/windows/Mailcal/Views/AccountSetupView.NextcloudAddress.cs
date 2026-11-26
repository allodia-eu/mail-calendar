// The manual form's account types, as the core offers them, and its Nextcloud pane
// (docs/nextcloud-sign-in.md rule 2): the server's address alone, which the core asks whether it is
// Nextcloud before the browser opens, and the offer to sign in anyway where it could not tell.
//
// The flight and the completion are the model's (MailboxModel.Nextcloud.cs); what the pane starts
// and offers is SetupNextcloudAddress's, which the unit suite drives.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    private readonly SetupNextcloudAddress _nextcloudAddress = new();

    // What the pane says while a sign-in it started is out.
    private NextcloudWait _nextcloudAddressWait;

    /// <summary>Lays the account types out as the core offers them, in its order. A type it does
    /// not offer is left out of the form, and detection never routes to one.</summary>
    private void ArrangeAccountTypes()
    {
        var choices = new Dictionary<DetectTab, RadioButton>
        {
            [DetectTab.Imap] = ImapChoice,
            [DetectTab.Jmap] = JmapChoice,
            [DetectTab.Microsoft] = MicrosoftChoice,
            [DetectTab.Google] = GoogleChoice,
            [DetectTab.Nextcloud] = NextcloudChoice,
            [DetectTab.Dav] = DavChoice,
        };
        AccountTypeRow.Children.Clear();
        foreach (var kind in MailcalBindingsMethods.ManualSetupKinds())
        {
            AccountTypeRow.Children.Add(choices[AccountDetectForm.Tab(kind)]);
        }
    }

    /// <summary>Back to an empty pane with nothing asked, when the form starts again.</summary>
    private void ResetNextcloudAddress()
    {
        _nextcloudAddress.Reset();
        _nextcloudAddressWait = NextcloudWait.None;
        if (NextcloudServer is not null)
        {
            NextcloudServer.Text = string.Empty;
        }
        UpdateNextcloudAddress();
    }

    private void OnNextcloudServerChanged(object sender, TextChangedEventArgs e) => UpdateNextcloudAddress();

    // Enter in the address field signs in, as the button would, only while the button would.
    private void OnNextcloudServerKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == Windows.System.VirtualKey.Enter && NextcloudAddressSignInButton.IsEnabled)
        {
            e.Handled = true;
            SignInNextcloudAddress(anyway: false);
        }
    }

    private void OnSignInNextcloudAddress(object sender, RoutedEventArgs e) => SignInNextcloudAddress(anyway: false);

    private void OnSignInNextcloudAnyway(object sender, RoutedEventArgs e) => SignInNextcloudAddress(anyway: true);

    private async void SignInNextcloudAddress(bool anyway)
    {
        if (Model is not { } model || !SetupNextcloudAddress.CanSignIn(NextcloudServer.Text))
        {
            return;
        }
        var start = SetupNextcloudAddress.Start(NextcloudServer.Text, anyway);
        _nextcloudAddress.Started();
        UpdateNextcloudAddress();
        var end = await model.SetupNextcloudSignInAsync(start, wait =>
        {
            _nextcloudAddressWait = wait;
            UpdateNextcloudAddress();
        });
        _nextcloudAddress.Ended(start, end);
        UpdateNextcloudAddress();
    }

    /// <summary>Draws the pane: the sign-in while the field holds an address, the wait while a
    /// sign-in is out, and the offer to try anyway while the field holds the address the core could
    /// not tell was Nextcloud.</summary>
    private void UpdateNextcloudAddress()
    {
        // The tree is still being built when the first fields raise their events.
        if (NextcloudUnsurePanel is null || NextcloudServer is null)
        {
            return;
        }
        var waiting = _nextcloudAddressWait != NextcloudWait.None;
        var free = !waiting && Model is { IsSubmitting: false };
        NextcloudAddressSignInButton.IsEnabled = free && SetupNextcloudAddress.CanSignIn(NextcloudServer.Text);
        NextcloudAddressWaitingPanel.Visibility = waiting ? Visibility.Visible : Visibility.Collapsed;
        NextcloudAddressWaitingRing.IsActive = waiting;
        NextcloudAddressWaitingText.Text = _nextcloudAddressWait == NextcloudWait.Checking
            ? L10n.SetupNextcloudChecking()
            : L10n.NextcloudSignInWaiting();
        var unsure = _nextcloudAddress.UnsureFor(NextcloudServer.Text);
        NextcloudUnsurePanel.Visibility = unsure is null ? Visibility.Collapsed : Visibility.Visible;
        NextcloudUnsureText.Text = unsure is null ? string.Empty : L10n.SetupNextcloudUnsure(unsure);
        NextcloudTryAnywayButton.IsEnabled = free;
    }
}
