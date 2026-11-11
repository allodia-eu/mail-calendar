// Settings → Accounts: what the person's other devices have to say, above their own mail accounts.
// Its Apple, Android and Linux twins draw the same three things in the same order, keep the states
// and the wording in step.
//
// It sits in Accounts rather than in the Allodia-account category because what it is about is mail
// accounts: one arriving is an account to set up, and that is where somebody looks for it.
//
// Its own partial, like every other category here, so SettingsDialog.cs stays clear of the
// 500-line limit.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>A pass started or ended. Only the Accounts list draws the card, so an account's page
    /// and every other category are left alone; the list is drawn again when its fingerprint,
    /// the card's included, moved.</summary>
    private void OnOtherDevices()
    {
        if (_accountPage is null && CurrentCategory() == "accounts")
        {
            OnAccountsChanged();
        }
    }

    /// <summary>
    /// The section, or null when there is nothing to say, including before the first pass has
    /// answered, which must not look like a pass that found nothing.
    /// </summary>
    private UIElement? BuildAllodiaSync(AllodiaDevices? devices)
    {
        if (devices is null || devices.NothingFound)
        {
            return null;
        }
        var panel = new StackPanel { Spacing = 6 };
        panel.Children.Add(Heading(L10n.SettingsAllodiaSyncHeading()));
        panel.Children.Add(Description(L10n.SettingsAllodiaSyncDescription()));
        if (devices.Checking)
        {
            var busy = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            busy.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
            busy.Children.Add(new TextBlock
            {
                Text = L10n.SettingsAllodiaSyncChecking(),
                Opacity = 0.7,
            });
            panel.Children.Add(busy);
        }
        foreach (var offer in devices.Offers)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            row.Children.Add(new TextBlock
            {
                Text = offer.Email,
                VerticalAlignment = VerticalAlignment.Center,
            });
            var setUp = new Button { Content = L10n.SettingsAllodiaSyncSetUp() };
            var taken = offer;
            setUp.Click += (_, _) => StartAddAccount(taken);
            row.Children.Add(setUp);
            panel.Children.Add(row);
        }
        // Both are questions, and the only answer this device can act on today is "keep what I
        // have". Applying the other side's settings needs a path for editing a connected
        // account's server details, which does not exist yet.
        foreach (var question in devices.Questions)
        {
            panel.Children.Add(Question(question));
        }
        if (devices.Failure is { } failure)
        {
            panel.Children.Add(BuildAllodiaFailure(failure));
        }
        return panel;
    }

    /// <summary>
    /// What a failed pass is allowed to put on screen.
    /// </summary>
    /// <remarks>
    /// The core's typed answer decides, never the failure text. A grant that predates a permission
    /// and one the service revoked are different sentences with different remedies, and everything
    /// else is a bad afternoon that says nothing about either, so it gets one plain line and the
    /// detail goes to the diagnostic log alone.
    /// <para>
    /// This is what stops the next unclassified error reaching a person the way `oauth endpoint
    /// error: invalid_scope, unable to issue scope mailcal:accounts:read` did: there is no longer
    /// a path from an exception's text to the screen.
    /// </para>
    /// </remarks>
    private UIElement BuildAllodiaFailure(AllodiaSyncFailure failure)
    {
        switch (failure)
        {
            // An offer, not an error: the person is signed in, and one feature is asleep.
            case AllodiaSyncFailure.NeedsReauth:
                return AllodiaSignInAgain(
                    L10n.SettingsAllodiaReauth(), L10n.SettingsAllodiaReauthHint(), L10n.SettingsAllodiaReauthAction());
            case AllodiaSyncFailure.SignedOut:
                return AllodiaSignInAgain(
                    L10n.SettingsAllodiaSignedOutElsewhere(),
                    L10n.SettingsAllodiaSignedOutElsewhereHint(),
                    L10n.SettingsAllodiaSignIn());
            default:
            {
                var line = Description(L10n.SettingsAllodiaSyncUnavailable());
                line.Opacity = 1;
                line.Foreground = (Microsoft.UI.Xaml.Media.Brush)
                    _brushes.Of(ThemePalette.Critical);
                return line;
            }
        }
    }

    /// <summary>
    /// The ordinary sign-in offered again, which is the whole of the remedy for both states, with
    /// what the last sign-in came to beneath it.
    /// </summary>
    /// <remarks>
    /// No separate flow: the sign-in asks for the full current scope set every time, so re-running
    /// it is what widens a grant that predates one, and what replaces a grant the service refused.
    /// The core runs a pass once it takes, so the account list is there before the person looks
    /// for it.
    /// </remarks>
    private StackPanel AllodiaSignInAgain(string heading, string hint, string action)
    {
        var panel = new StackPanel { Spacing = 6 };
        panel.Children.Add(Heading(heading));
        panel.Children.Add(Description(hint));
        if (_model.AllodiaCard() is { } card)
        {
            panel.Children.Add(ControlComponents.Button(
                action, card.SignIn, "AllodiaDevicesSignIn", () => _model.SignInToAllodia()));
            if (AllodiaCardRules.SignInSaid(card.Failure, CoreErrorCopy.AllodiaWords()) is { } said)
            {
                panel.Children.Add(CriticalLine(said));
            }
        }
        return panel;
    }

    /// <summary>The Allodia card moved while the Accounts list offers its sign-in again: the
    /// offer's button and what it says beneath are the card's, so the list is drawn again.</summary>
    private void OnAllodiaChangedOnAccounts()
    {
        if (!_rebuilding && _accountPage is null && CurrentCategory() == "accounts"
            && _model.AllodiaDevices()?.Failure is AllodiaSyncFailure.NeedsReauth or AllodiaSyncFailure.SignedOut)
        {
            ShowCategory("accounts");
        }
    }

    /// <summary>
    /// Opens the setup form on the offered address. Settings closes first: the form opens in a
    /// dialog of its own, and WinUI holds one dialog open at a time.
    /// </summary>
    private void StartAddAccount(AllodiaAccountOffer offer)
    {
        Hide();
        _model.BeginAddAccount(offer.Email, offer);
    }

    /// <summary>One account changed or removed elsewhere, and the answer this device can give:
    /// keep what it has, which pauses the account's sharing as a change on its page.</summary>
    private UIElement Question(AllodiaDevicesQuestion question)
    {
        var panel = new StackPanel { Spacing = 4 };
        panel.Children.Add(Description(question.Elsewhere == AllodiaElsewhere.Changed
            ? L10n.SettingsAllodiaChangedElsewhere(question.Email)
            : L10n.SettingsAllodiaRemovedElsewhere(question.Email)));
        var id = question.AccountId;
        panel.Children.Add(ControlComponents.Button(
            L10n.SettingsAllodiaKeepLocal(), question.Keep, $"AllodiaKeep_{id}",
            () => _model.ChangeAccount(new AccountPageIntent.SetSharing(id, AllodiaAccountSyncMode.Paused))));
        if (question.Failed is { } failed)
        {
            panel.Children.Add(SharingFailed(failed, $"AllodiaKeep_{id}_Failed"));
        }
        return panel;
    }
}
