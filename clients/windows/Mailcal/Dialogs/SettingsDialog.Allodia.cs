// Settings → Allodia account: the whole category, and the only place the app draws one. Its Apple,
// Android and Linux twins are AllodiaAccountSettings.swift, SettingsAllodia.kt and
// settings/allodia.rs, keep the states and the wording in step.
//
// A category of its own rather than a card under Accounts, because an Allodia account is not a mail
// account: no mailbox, no switcher entry, and a token issued for it cannot touch anyone's mail. The
// setup wizard never offers it.
//
// The CATEGORY is dropped when this build carries no route, so a build from source has no such
// screen at all, absent, never present-and-broken (SettingsDialog.Categories).
//
// Drawn from the core's AllodiaCard, which says who is signed in, the sign-in out and what the last
// sign-in or sign-out came to (AllodiaCardRules); the dialog keeps none of it.
//
// Its own partial, like the Accounts category itself, so SettingsDialog.cs stays clear of the
// 500-line limit.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // What the category was last drawn from, so a signal that moved none of it leaves the panel,
    // and the button under the pointer, alone.
    private string? _allodiaDrawn;
    // Who the card named when last drawn or signalled, and whether a sign-in was out.
    private (string? Email, bool SigningIn) _allodiaSeen;

    // The card, or null in a build that carries no Allodia registration, which is the ordinary
    // answer for a build from source, and draws nothing at all rather than a dead button.
    private UIElement? BuildAllodiaAccount()
    {
        if (!MailcalBindingsMethods.AllodiaSignInAvailable() || _model.AllodiaCard() is not { } card)
        {
            return null;
        }
        _allodiaDrawn = card.ToString();
        _allodiaSeen = (AllodiaCardRules.Account(card)?.Email, card.Browser is not null);
        var panel = new StackPanel { Spacing = 6 };
        panel.Children.Add(Heading(L10n.SettingsAllodiaHeading()));
        panel.Children.Add(Description(L10n.SettingsAllodiaDescription()));
        panel.Children.Add(AllodiaState(card));
        if (AllodiaCardRules.Said(card.Failure, CoreErrorCopy.AllodiaWords()) is { } failure)
        {
            panel.Children.Add(CriticalLine(failure));
        }
        // The subscription sits under the account it belongs to, and is drawn only while somebody
        // is signed in (SettingsDialog.Subscription.cs).
        if (BuildAllodiaSubscription(card) is { } subscription)
        {
            panel.Children.Add(subscription);
        }
        return panel;
    }

    private UIElement AllodiaState(AllodiaCard card)
    {
        switch (AllodiaCardRules.Shows(card))
        {
            case AllodiaCardShows.SigningIn:
            {
                var browser = card.Browser!;
                var busy = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
                busy.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
                busy.Children.Add(new TextBlock
                {
                    Text = L10n.SettingsAllodiaSigningIn(),
                    Opacity = 0.7,
                    VerticalAlignment = VerticalAlignment.Center,
                });
                busy.Children.Add(ControlComponents.Button(
                    L10n.ActionCancel(), browser.Cancel, "AllodiaCancel", () => _model.CancelAllodiaSignIn(browser)));
                return busy;
            }
            case AllodiaCardShows.SignedIn:
                return AllodiaSignedIn(card, AllodiaCardRules.Account(card)!);
            case AllodiaCardShows.Ended:
                return AllodiaEnded(card, AllodiaCardRules.Account(card)!.Email);
            default:
            {
                // Both routes. Someone who has no account and someone returning to one need
                // different pages, and a lone "Sign in" sends the first of them through a form
                // asking for a password they never set.
                var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
                buttons.Children.Add(ControlComponents.Button(
                    L10n.SettingsAllodiaSignIn(), card.SignIn, "AllodiaSignIn", () => _model.SignInToAllodia()));
                buttons.Children.Add(ControlComponents.Button(
                    L10n.SettingsAllodiaCreate(), card.Create, "AllodiaCreate", () => _model.SignInToAllodia(create: true)));
                return buttons;
            }
        }
    }

    private StackPanel AllodiaSignedIn(AllodiaCard card, AllodiaAccount account)
    {
        var panel = new StackPanel { Spacing = 6 };
        // The name is what the person recognises, but the address is what identifies the account,
        // so the address is always shown, and the name only when the service holds one.
        if (!string.IsNullOrWhiteSpace(account.Name))
        {
            panel.Children.Add(new TextBlock { Text = account.Name });
        }
        panel.Children.Add(Description(L10n.SettingsAllodiaSignedIn(account.Email)));
        // Managing and deleting are the same page, named twice on purpose: an account someone can
        // create has to offer deletion somewhere findable, and "Manage account" is not the word
        // anybody looks for when they want out.
        var manage = new Button { Content = L10n.SettingsAllodiaManage() };
        manage.Click += (_, _) => _model.OpenAllodiaAccountPage();
        panel.Children.Add(manage);
        panel.Children.Add(Description(L10n.SettingsAllodiaManageHint()));
        var delete = new Button { Content = L10n.SettingsAllodiaDelete() };
        delete.Click += (_, _) => _model.OpenAllodiaAccountPage();
        panel.Children.Add(delete);
        panel.Children.Add(AllodiaSignOutButton(card));
        return panel;
    }

    // In place of the signed-in card, and without Manage account: the page would ask for a sign-in
    // too. The stored grant is kept until the person acts, which is what still names the address.
    private StackPanel AllodiaEnded(AllodiaCard card, string email)
    {
        var panel = new StackPanel { Spacing = 6 };
        panel.Children.Add(new TextBlock
        {
            Text = L10n.SettingsAllodiaSessionEnded(email),
            TextWrapping = TextWrapping.Wrap,
        });
        panel.Children.Add(Description(L10n.SettingsAllodiaSessionEndedHint()));
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        buttons.Children.Add(ControlComponents.Button(
            L10n.SettingsAllodiaReauthAction(), card.SignIn, "AllodiaSignInAgain", () => _model.SignInToAllodia()));
        buttons.Children.Add(AllodiaSignOutButton(card));
        panel.Children.Add(buttons);
        return panel;
    }

    private StackPanel AllodiaSignOutButton(AllodiaCard card) => ControlComponents.Button(
        L10n.SettingsAllodiaSignOut(), card.SignOut, "AllodiaSignOut", _model.SignOutOfAllodia);

    // A line saying what did not work.
    private TextBlock CriticalLine(string text)
    {
        var line = Description(text);
        line.Opacity = 1;
        line.Foreground = _brushes.Of(ThemePalette.Critical);
        return line;
    }

    // The card moved (Surface.Allodia). The subscription read is about the account it was made
    // for, so it is forgotten when the account named moves or a sign-in ends: signing in again is
    // the remedy the read's reauth state offers, and leaves the same account behind. The category
    // is drawn again when it is the one open and what it draws moved, and so is an Accounts list
    // offering the sign-in again.
    private void OnAllodiaChanged()
    {
        if (_model.AllodiaCard() is not { } card)
        {
            return;
        }
        (string? Email, bool SigningIn) seen = (AllodiaCardRules.Account(card)?.Email, card.Browser is not null);
        if (seen.Email != _allodiaSeen.Email || (_allodiaSeen.SigningIn && !seen.SigningIn))
        {
            ForgetAllodiaSubscriptionRead();
        }
        _allodiaSeen = seen;
        OnAllodiaChangedOnAccounts();
        if (_rebuilding || CurrentCategory() != "allodia" || card.ToString() == _allodiaDrawn)
        {
            return;
        }
        ShowCategory("allodia");
    }
}
