// Signing in with Nextcloud from setup (docs/nextcloud-sign-in.md rule 2): from the found card for
// a calendar and contacts server detection recognised as Nextcloud, where the sign-in leads and the
// password waits behind "Use a password instead" as the IMAP card's sign-in does, and from the
// manual calendar and contacts form once the server typed there answers as Nextcloud, where the
// offer is drawn above the fields and the fields keep what was typed.
//
// The flight and the completion are the model's (MailboxModel.Nextcloud.cs); which server and uses
// a press signs in at, and when the manual form asks, are SetupNextcloudOffer's, which the unit
// suite drives.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    private readonly SetupNextcloudOffer _nextcloudOffer = new();

    // The found card's server is Nextcloud.
    private bool _davNextcloud;

    // "Use a password instead" was pressed on that card.
    private bool _davPasswordRevealed;

    // The browser holds a sign-in started here.
    private bool _nextcloudWaiting;

    /// <summary>Takes what detection said about the card's server.</summary>
    private void AdoptNextcloud(DetectRoute route)
    {
        _davNextcloud = route.NextcloudSignIn && route.Tab == DetectTab.Dav && !route.IsManual;
        _davPasswordRevealed = false;
    }

    /// <summary>"Set up manually" from the card: detection asked already, so the manual form
    /// offers the sign-in at the server it names without asking again.</summary>
    private void NextcloudManualFromFound()
    {
        if (_davNextcloud && SetupNextcloudOffer.ManualServer(DavCaldavUrl.Text, DavCarddavUrl.Text) is { } server)
        {
            _nextcloudOffer.Known(server);
        }
        _davNextcloud = false;
    }

    /// <summary>Back to nothing asked, when the form starts again.</summary>
    private void ResetNextcloud()
    {
        _nextcloudOffer.Reset();
        _davNextcloud = false;
        _davPasswordRevealed = false;
        _nextcloudWaiting = false;
        UpdateNextcloud();
    }

    private void OnDavServerChanged(object sender, TextChangedEventArgs e) => UpdateNextcloud();

    // A server field left on the manual form: ask its server, off the UI thread, once per server.
    // Nothing on screen moves while it is asked, so what is being typed is left alone.
    private async void OnDavServerLeft(object sender, RoutedEventArgs e)
    {
        if (Model is not { } model || _detectedTab == DetectTab.Dav
            || _nextcloudOffer.Ask(DavCaldavUrl.Text, DavCarddavUrl.Text) is not { } server)
        {
            return;
        }
        TestAgent.Signal(DavCaldavUrl, "asked whether the server is Nextcloud");
        _nextcloudOffer.Answered(server, await model.OffersNextcloudSignInAsync(server));
        UpdateNextcloud();
    }

    private void OnUseDavPassword(object sender, RoutedEventArgs e)
    {
        _davPasswordRevealed = true;
        UpdateNextcloud();
        UpdateCanConnect();
        DavPassword.Focus(FocusState.Programmatic);
    }

    private async void OnSignInNextcloud(object sender, RoutedEventArgs e)
    {
        if (Model is not { } model)
        {
            return;
        }
        NextcloudStart? start;
        if (_detectedTab == DetectTab.Dav)
        {
            if (FlagMissingServer())
            {
                return;
            }
            start = SetupNextcloudOffer.Found(ChosenUses());
        }
        else
        {
            start = _nextcloudOffer.OfferedFor(DavCaldavUrl.Text, DavCarddavUrl.Text) is { } server
                ? SetupNextcloudOffer.Manual(server, DavCaldavUrl.Text, DavCarddavUrl.Text)
                : null;
        }
        if (start is null)
        {
            return;
        }
        await model.SetupNextcloudSignInAsync(start, waiting =>
        {
            _nextcloudWaiting = waiting;
            UpdateNextcloud();
        });
        UpdateNextcloud();
        UpdateCanConnect();
    }

    /// <summary>Draws the offer as the card or the form has it.</summary>
    private void UpdateNextcloud()
    {
        // The tree is still being built when the first fields raise their events.
        if (NextcloudPanel is null || DavPasswordPanel is null || DavCarddavUrl is null)
        {
            return;
        }
        var card = _detectedTab == DetectTab.Dav;
        var offered = card
            ? _davNextcloud
            : _nextcloudOffer.OfferedFor(DavCaldavUrl.Text, DavCarddavUrl.Text) is not null;
        NextcloudPanel.Visibility = offered ? Visibility.Visible : Visibility.Collapsed;
        // On the card the password waits behind "Use a password instead" (docs/mail-oauth.md
        // rule 2), and Connect, which submits it, goes with it.
        var behind = card && _davNextcloud && !_davPasswordRevealed;
        DavPasswordPanel.Visibility = behind ? Visibility.Collapsed : Visibility.Visible;
        DavUsePasswordButton.Visibility = behind ? Visibility.Visible : Visibility.Collapsed;
        NextcloudSignInButton.IsEnabled = !_nextcloudWaiting && Model is { IsSubmitting: false };
        // Primary until a password is on screen beside it; then Connect leads. Assigned only on a
        // change, because a new style re-templates the button under a focused pointer.
        var style = (Style)Application.Current.Resources[behind ? "AccentButtonStyle" : "DefaultButtonStyle"];
        if (!ReferenceEquals(NextcloudSignInButton.Style, style))
        {
            NextcloudSignInButton.Style = style;
        }
        NextcloudWaitingPanel.Visibility = _nextcloudWaiting ? Visibility.Visible : Visibility.Collapsed;
        NextcloudWaitingRing.IsActive = _nextcloudWaiting;
        if (DavChoice.IsChecked == true)
        {
            ConnectButton.Visibility = behind ? Visibility.Collapsed : Visibility.Visible;
        }
    }
}
