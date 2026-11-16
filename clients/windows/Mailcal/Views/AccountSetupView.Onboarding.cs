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
// How far it has got, and what the last sign-in came to, is the core's AllodiaCard: the view keeps
// none of it.
//
// Its own partial, like the JMAP sign-in beside it, so the code-behind stays clear of the 500-line
// limit.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    // What the panel was last drawn from, so a Settings signal that changed none of it leaves the
    // panel, and the button under the pointer, alone.
    private string? _onboardingDrawn;

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
        _onboardingDrawn = OnboardingFingerprint();
        OnboardingAllodiaPanel.Children.Clear();
        var firstRun = Model?.NeedsSetup == true;
        if (!MailcalBindingsMethods.AllodiaSignInAvailable())
        {
            OnboardingAllodiaPanel.Visibility = Visibility.Collapsed;
            return;
        }
        if (!firstRun || Model?.AllodiaCard() is not { } card)
        {
            var outstanding = Model?.AllodiaDevices()?.Offers ?? System.Array.Empty<AllodiaAccountOffer>();
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
        OnboardingAllodiaPanel.Children.Add(OnboardingContent(card));
        if (AllodiaCardRules.Said(card.Failure, CoreErrorCopy.AllodiaWords()) is { } failure)
        {
            OnboardingAllodiaPanel.Children.Add(new TextBlock
            {
                Text = failure,
                TextWrapping = TextWrapping.Wrap,
                Foreground = ThemePalette.Brush(ThemePalette.Critical(this.IsDark())),
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
            Background = ThemePalette.Brush(ThemePalette.Divider(this.IsDark())),
        });
        OnboardingAllodiaPanel.Children.Add(new TextBlock
        {
            Text = L10n.SetupAllodiaDivider(),
            Foreground = ThemePalette.Brush(ThemePalette.SecondaryText(this.IsDark())),
        });
    }

    private UIElement OnboardingContent(AllodiaCard card)
    {
        switch (card.Onboarding)
        {
            case AllodiaOnboarding.SigningIn:
            {
                var busy = Busy(L10n.SettingsAllodiaSigningIn());
                // The redirect is a custom scheme, which has no socket to drop, so a browser
                // closed on the sign-in sends nothing back. Cancel is the way back to the card,
                // drawn once the core says the sign-in has run long enough to need it.
                if (AllodiaCardRules.OnboardingCancel(card) is { } cancel && card.Browser is { } browser)
                {
                    busy.Children.Add(ControlComponents.Button(
                        L10n.ActionCancel(), cancel, "OnboardingAllodiaCancel", () => Model?.CancelAllodiaSignIn(browser)));
                }
                return busy;
            }
            // The pass that follows needs no way back: no browser, and it ends on its own.
            case AllodiaOnboarding.Checking:
                return Busy(L10n.SettingsAllodiaSyncChecking());
            // Signed in and asked. Offers become the fast route; none means this account has no
            // mail accounts on it yet.
            case AllodiaOnboarding.SignedIn:
                return Offers();
            default:
                return Recommendation(card);
        }
    }

    private static StackPanel Busy(string text)
    {
        var busy = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        busy.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
        busy.Children.Add(new TextBlock
        {
            Text = text,
            VerticalAlignment = VerticalAlignment.Center,
            Opacity = 0.7,
        });
        return busy;
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
        // A pass that has not answered, or one that failed, says nothing. Reporting it as an
        // empty account states a result nobody has, so "none" is said only when the core says
        // the pass found nothing.
        if (Model?.AllodiaDevices() is not { } devices)
        {
            return panel;
        }
        var offers = devices.Offers;
        if (devices.NothingFound)
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
                Foreground = ThemePalette.Brush(ThemePalette.SecondaryText(this.IsDark())),
            });
            return panel;
        }
        if (offers.Length > 0)
        {
            panel.Children.Add(OfferRows(offers));
        }
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
            // The route Settings → Accounts opens an offer on (ResetToDetect).
            setUp.Click += (_, _) =>
            {
                DetectEmail.Text = taken.Email;
                ContinueButton.IsEnabled = true;
                var offered = MailcalBindingsMethods.OfferedSetup(taken);
                Model?.SetupDetected(offered);
                ApplyRoute(AccountDetectForm.Route(offered, taken.Email));
            };
            row.Children.Add(setUp);
            panel.Children.Add(row);
        }
        return panel;
    }

    private UIElement Recommendation(AllodiaCard card)
    {
        var panel = new StackPanel { Spacing = 12 };
        var content = new StackPanel { Spacing = 6 };
        content.Children.Add(new TextBlock
        {
            Text = L10n.SetupAllodiaRecommended(),
            Foreground = ThemePalette.Brush(ThemePalette.AccentText(this.IsDark())),
        });
        content.Children.Add(new TextBlock
        {
            Text = L10n.SetupAllodiaTitle(),
            Style = (Style)Application.Current.Resources["SubtitleTextBlockStyle"],
        });
        content.Children.Add(new TextBlock
        {
            Text = L10n.SetupAllodiaSubtitle(),
            TextWrapping = TextWrapping.Wrap,
            Opacity = 0.8,
        });
        content.Children.Add(ControlComponents.Button(
            L10n.SetupAllodiaCreate(),
            card.Create,
            "OnboardingAllodiaCreate",
            () => Model?.SignInToAllodia(create: true),
            (Style)Application.Current.Resources["AccentButtonStyle"]));

        // One control rather than a heading beside a button, so a screen reader announces the offer
        // and its action together, and the name carries the ACTION, never the "Recommended"
        // marker.
        var group = new Border
        {
            Child = content,
            Padding = new Thickness(12),
            CornerRadius = new CornerRadius(8),
            Background = ThemePalette.Brush(ThemePalette.CardBackground(this.IsDark())),
        };
        AutomationProperties.SetName(group, L10n.SetupAllodiaTitle());
        AutomationProperties.SetHelpText(group, L10n.SetupAllodiaSubtitle());
        panel.Children.Add(group);

        // One line, not a second control of equal weight.
        var signIn = new HyperlinkButton { Content = L10n.SetupAllodiaHaveOne() };
        signIn.Click += (_, _) => Model?.SignInToAllodia();
        panel.Children.Add(signIn);
        return panel;
    }

    /// <summary>The Allodia card or the other devices' pass moved.</summary>
    private void OnOnboardingSignal()
    {
        if (OnboardingFingerprint() != _onboardingDrawn)
        {
            RenderOnboarding();
        }
    }

    private string OnboardingFingerprint() => string.Join('\n', [
        $"{Model?.NeedsSetup}/{AllodiaCardRules.OnboardingFingerprint(Model?.AllodiaCard())}",
        .. AccountSettingsRules.DevicesFingerprint(Model?.AllodiaDevices()),
    ]);
}
