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
// Its own partial, like the Accounts category itself, so SettingsDialog.cs stays clear of the
// 500-line limit.

using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The browser hop is outstanding: the card shows progress and a way out instead of a button
    // that would start a second flow and discard the first one's verifier.
    private bool _allodiaSigningIn;
    // The last sign-in or sign-out failure, in the service's own words, or null.
    private string? _allodiaFailure;

    // The card, or null in a build that carries no Allodia registration, which is the ordinary
    // answer for a build from source, and draws nothing at all rather than a dead button.
    private UIElement? BuildAllodiaAccount()
    {
        if (!MailcalBindingsMethods.AllodiaSignInAvailable())
        {
            return null;
        }
        var panel = new StackPanel { Spacing = 6 };
        panel.Children.Add(Heading(L10n.SettingsAllodiaHeading()));
        panel.Children.Add(Description(L10n.SettingsAllodiaDescription()));
        panel.Children.Add(AllodiaState());
        if (_allodiaFailure is { } failure)
        {
            var error = Description(L10n.SettingsAllodiaFailed(failure));
            error.Opacity = 1;
            error.Foreground =
                (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources["SystemFillColorCriticalBrush"];
            panel.Children.Add(error);
        }
        return panel;
    }

    // Signing in → progress and Cancel; signed in → who, and a way out; otherwise → Sign in.
    private UIElement AllodiaState()
    {
        if (_allodiaSigningIn)
        {
            var busy = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            busy.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
            busy.Children.Add(new TextBlock { Text = L10n.SettingsAllodiaSigningIn(), Opacity = 0.7 });
            var cancel = new Button { Content = L10n.ActionCancel() };
            cancel.Click += (_, _) => _model.CancelAllodiaSignIn();
            busy.Children.Add(cancel);
            return busy;
        }
        var panel = new StackPanel { Spacing = 6 };
        if (_model.SignedInAllodiaAccount() is { } account)
        {
            // The name is what the person recognises, but the address is what identifies the
            // account, so the address is always shown, and the name only when the service holds
            // one.
            if (!string.IsNullOrWhiteSpace(account.Name))
            {
                panel.Children.Add(new TextBlock { Text = account.Name });
            }
            panel.Children.Add(Description(L10n.SettingsAllodiaSignedIn(account.Email)));
            // Managing and deleting are the same page, named twice on purpose: an account
            // someone can create has to offer deletion somewhere findable, and "Manage account" is
            // not the word anybody looks for when they want out.
            var manage = new Button { Content = L10n.SettingsAllodiaManage() };
            manage.Click += (_, _) => _model.OpenAllodiaAccountPage();
            panel.Children.Add(manage);
            panel.Children.Add(Description(L10n.SettingsAllodiaManageHint()));
            var delete = new Button { Content = L10n.SettingsAllodiaDelete() };
            delete.Click += (_, _) => _model.OpenAllodiaAccountPage();
            panel.Children.Add(delete);
            var signOut = new Button { Content = L10n.SettingsAllodiaSignOut() };
            // The account is forgotten in memory whatever the store does, so the rebuild re-reads
            // rather than assuming: a delete that failed leaves the app signed out and says why.
            signOut.Click += (_, _) => Apply(() => _allodiaFailure = _model.SignOutOfAllodia());
            panel.Children.Add(signOut);
            return panel;
        }
        // Both routes. Someone who has no account and someone returning to one need different
        // pages, and a lone "Sign in" sends the first of them through a form asking for a password
        // they never set.
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var signIn = new Button { Content = L10n.SettingsAllodiaSignIn() };
        signIn.Click += (_, _) => StartAllodiaSignIn(create: false);
        buttons.Children.Add(signIn);
        var create = new Button { Content = L10n.SettingsAllodiaCreate() };
        create.Click += (_, _) => StartAllodiaSignIn(create: true);
        buttons.Children.Add(create);
        panel.Children.Add(buttons);
        return panel;
    }

    private void StartAllodiaSignIn(bool create)
    {
        if (_allodiaSigningIn)
        {
            return;
        }
        _allodiaFailure = null;
        // Re-render into the busy state first, so the browser opens over a card that already says
        // what is happening; the flow itself runs on after the panel is replaced.
        Apply(() => _allodiaSigningIn = true);
        _ = RunAllodiaSignInAsync(create);
    }

    private async Task RunAllodiaSignInAsync(bool create)
    {
        var failure = await _model.SignInToAllodiaAsync(create);
        _allodiaSigningIn = false;
        _allodiaFailure = failure;
        // No state to pass: the card re-reads who is signed in from the core.
        Apply(() => { });
    }
}
