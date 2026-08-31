// First run: the Allodia-account recommendation, above the address field.
//
// docs/onboarding.md is the contract and decides the order, the card, the way back for someone who
// already has one, a divider naming what follows, then the address field. Its Apple, Android and
// Linux twins draw the same four things in the same order.
//
// Three rules it is easy to break silently:
//
//   * A build with no Allodia registration loses the card, the sign-in line AND the divider
//     together. A lone "or connect directly" heading under nothing is the tell that the wrong thing
//     was gated.
//   * The copy may not out-run the capability matrix: phone and computer, never web.
//   * The card claims the account LIST and nothing else, never the mail, never a password.
//
// Its own partial, like the JMAP sign-in beside it, so the code-behind stays clear of the 500-line
// limit.

using System.Threading.Tasks;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    // The browser hop is outstanding: no button to press again, which would discard the first
    // flow's verifier.
    private bool _onboardingSigningIn;
    private string? _onboardingFailure;

    /// <summary>
    /// Rebuilds the panel above the address field, or hides it.
    /// </summary>
    /// <remarks>
    /// Hidden whole: the card, the line and the divider are one thing, and gating only the first
    /// leaves a heading naming a choice nobody was offered.
    ///
    /// The card and the offers part company on the second account. The card is a pitch and is
    /// asked once: somebody who signed in has decided. The offers are not a pitch, they are
    /// accounts they already have, and gating them with the card left the second of three linked
    /// accounts reachable only from a Settings page (docs/onboarding.md).
    /// </remarks>
    private void RenderOnboarding()
    {
        OnboardingAllodiaPanel.Children.Clear();
        var firstRun = Model?.NeedsSetup == true;
        if (!MailcalBindingsMethods.AllodiaSignInAvailable())
        {
            OnboardingAllodiaPanel.Visibility = Visibility.Collapsed;
            return;
        }
        if (!firstRun)
        {
            var outstanding = Model?.AllodiaSync?.Offers ?? System.Array.Empty<AllodiaAccountOffer>();
            if (outstanding.Length == 0)
            {
                OnboardingAllodiaPanel.Visibility = Visibility.Collapsed;
                return;
            }
            OnboardingAllodiaPanel.Visibility = Visibility.Visible;
            OnboardingAllodiaPanel.Children.Add(OfferRows(outstanding));
            AddOnboardingDivider();
            return;
        }
        OnboardingAllodiaPanel.Visibility = Visibility.Visible;
        OnboardingAllodiaPanel.Children.Add(OnboardingContent());
        if (_onboardingFailure is { } failure)
        {
            OnboardingAllodiaPanel.Children.Add(new TextBlock
            {
                Text = L10n.SettingsAllodiaFailed(failure),
                TextWrapping = TextWrapping.Wrap,
                Foreground = (Microsoft.UI.Xaml.Media.Brush)
                    Application.Current.Resources["SystemFillColorCriticalBrush"],
            });
        }
        AddOnboardingDivider();
    }

    /// <summary>What the address field below is, named.</summary>
    /// <remarks>
    /// Only ever under something: a lone "or connect directly" heading over nothing is the tell
    /// that a client gated the wrong half.
    /// </remarks>
    private void AddOnboardingDivider()
    {
        OnboardingAllodiaPanel.Children.Add(new Border
        {
            Height = 1,
            Background = (Microsoft.UI.Xaml.Media.Brush)
                Application.Current.Resources["DividerStrokeColorDefaultBrush"],
        });
        OnboardingAllodiaPanel.Children.Add(new TextBlock
        {
            Text = L10n.SetupAllodiaDivider(),
            Foreground = (Microsoft.UI.Xaml.Media.Brush)
                Application.Current.Resources["TextFillColorSecondaryBrush"],
        });
    }

    private UIElement OnboardingContent()
    {
        var signedIn = Model?.SignedInAllodiaAccount() is not null;
        if (_onboardingSigningIn || (signedIn && Model?.AllodiaSyncing == true))
        {
            var busy = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            busy.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
            busy.Children.Add(new TextBlock
            {
                Text = _onboardingSigningIn
                    ? L10n.SettingsAllodiaSigningIn()
                    : L10n.SettingsAllodiaSyncChecking(),
                VerticalAlignment = VerticalAlignment.Center,
                Opacity = 0.7,
            });
            if (_onboardingSigningIn)
            {
                // Closing the browser sends nothing back: the redirect is a custom scheme, which
                // has no socket to drop, so an abandoned sign-in would sit here until the core's
                // own cap. Cancel is the only way back to the card. The sync pass below needs
                // none, no browser, and it ends on its own.
                var cancel = new Button { Content = L10n.ActionCancel() };
                cancel.Click += (_, _) => Model?.CancelAllodiaSignIn();
                busy.Children.Add(cancel);
            }
            return busy;
        }
        // Signed in and asked. Offers become the fast route; none means this account has no mail
        // accounts on it yet.
        return signedIn ? Offers() : Recommendation();
    }

    /// <summary>
    /// What a signed-in person is offered, which for a first device is a sentence rather than rows.
    /// </summary>
    /// <remarks>
    /// The empty answer is the one worth drawing carefully. Nothing came back, the card is gone,
    /// and what is left under the divider is an address field the person has no reason to connect
    /// with the sign-in they just finished, it reads as the sign-in having failed. So the empty
    /// case says what happened and what to do (docs/onboarding.md).
    /// </remarks>
    private UIElement Offers()
    {
        var panel = new StackPanel { Spacing = 8 };
        // A pass that has not answered, one that failed on the network, says nothing. Reporting
        // it as an empty account states a result nobody has.
        if (Model?.AllodiaSync is not { } report)
        {
            return panel;
        }
        var offers = report.Offers;
        if (offers.Length == 0)
        {
            panel.Children.Add(new TextBlock
            {
                Text = L10n.SetupAllodiaNoneTitle(),
                Style = (Style)Application.Current.Resources["SubtitleTextBlockStyle"],
            });
            panel.Children.Add(new TextBlock
            {
                Text = L10n.SetupAllodiaNoneBody(),
                TextWrapping = TextWrapping.Wrap,
                Foreground = (Microsoft.UI.Xaml.Media.Brush)
                    Application.Current.Resources["TextFillColorSecondaryBrush"],
            });
            return panel;
        }
        panel.Children.Add(OfferRows(offers));
        return panel;
    }

    /// <summary>The accounts the person's other devices hold, as rows.</summary>
    /// <remarks>
    /// The button carries the whole record, not the address: the route comes from what the other
    /// device wrote down, which is the point of having synced it. Re-deriving it from the address
    /// spends a round trip to re-learn what is in front of us, and for a domain that publishes no
    /// autoconfig it finds less, dropping the person onto the manual form for an account another
    /// device set up without trouble. The password is still asked for here, because none travels.
    /// </remarks>
    private UIElement OfferRows(AllodiaAccountOffer[] offers)
    {
        var panel = new StackPanel { Spacing = 8 };
        panel.Children.Add(new TextBlock
        {
            Text = L10n.SettingsAllodiaSyncHeading(),
            Style = (Style)Application.Current.Resources["SubtitleTextBlockStyle"],
        });
        foreach (var offer in offers)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            row.Children.Add(new TextBlock
            {
                Text = offer.Email,
                VerticalAlignment = VerticalAlignment.Center,
            });
            var setUp = new Button { Content = L10n.SettingsAllodiaSyncSetUp() };
            var taken = offer;
            setUp.Click += (_, _) =>
            {
                DetectEmail.Text = taken.Email;
                ContinueButton.IsEnabled = true;
                ApplyRoute(AccountDetectForm.Route(
                    MailcalBindingsMethods.SetupFromOffer(taken)));
            };
            row.Children.Add(setUp);
            panel.Children.Add(row);
        }
        return panel;
    }

    private UIElement Recommendation()
    {
        var panel = new StackPanel { Spacing = 12 };
        var card = new StackPanel { Spacing = 6 };
        card.Children.Add(new TextBlock
        {
            Text = L10n.SetupAllodiaRecommended(),
            Foreground = (Microsoft.UI.Xaml.Media.Brush)
                Application.Current.Resources["AccentTextFillColorPrimaryBrush"],
        });
        card.Children.Add(new TextBlock
        {
            Text = L10n.SetupAllodiaTitle(),
            Style = (Style)Application.Current.Resources["SubtitleTextBlockStyle"],
        });
        card.Children.Add(new TextBlock
        {
            Text = L10n.SetupAllodiaSubtitle(),
            TextWrapping = TextWrapping.Wrap,
            Opacity = 0.8,
        });
        var create = new Button
        {
            Content = L10n.SetupAllodiaCreate(),
            Style = (Style)Application.Current.Resources["AccentButtonStyle"],
        };
        create.Click += (_, _) => _ = StartOnboardingSignInAsync(create: true);
        card.Children.Add(create);

        // One control rather than a heading beside a button, so a screen reader announces the offer
        // and its action together, and the name carries the ACTION, never the "Recommended"
        // marker.
        var group = new Border
        {
            Child = card,
            Padding = new Thickness(12),
            CornerRadius = new CornerRadius(8),
            Background = (Microsoft.UI.Xaml.Media.Brush)
                Application.Current.Resources["CardBackgroundFillColorDefaultBrush"],
        };
        AutomationProperties.SetName(group, L10n.SetupAllodiaTitle());
        AutomationProperties.SetHelpText(group, L10n.SetupAllodiaSubtitle());
        panel.Children.Add(group);

        // One line, not a second control of equal weight.
        var signIn = new HyperlinkButton { Content = L10n.SetupAllodiaHaveOne() };
        signIn.Click += (_, _) => _ = StartOnboardingSignInAsync(create: false);
        panel.Children.Add(signIn);
        return panel;
    }

    private async Task StartOnboardingSignInAsync(bool create)
    {
        if (_onboardingSigningIn || Model is null)
        {
            return;
        }
        _onboardingFailure = null;
        _onboardingSigningIn = true;
        RenderOnboarding();
        _onboardingFailure = await Model.SignInToAllodiaAsync(create);
        _onboardingSigningIn = false;
        RenderOnboarding();
        if (_onboardingFailure is null)
        {
            // What their other devices hold, before they are asked to type anything.
            await Model.SyncAllodiaAccountsAsync();
            RenderOnboarding();
        }
    }
}
