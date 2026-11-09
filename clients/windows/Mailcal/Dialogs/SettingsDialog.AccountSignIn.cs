// An account's page, how it signs in (docs/accounts.md rule 17), drawn from the core's record: the
// line saying how it signs in or what is wrong, the ways in through a browser, and the field a
// password is typed into. Which of them it offers is the core's. A browser sign-in is still this
// client's to run: Microsoft and Google through their own consent (AccountUses), a calendar and
// contacts account with Nextcloud, every other account through its own sign-in.
//
// A sign-in waiting on the browser leaves the buttons live, so a press supersedes it, and draws a
// Cancel that retires it (AccountSignInProgress); only a blocking core call disables them.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private bool _nextcloudClosedWired;

    // Where the page's sign-in is, which every drawing of the section reads.
    private readonly AccountSignInProgress _signInProgress = new();

    // The browser routes and the Cancel of the section drawn last, whose, and whether a change on
    // the page holds them; each step of the sign-in sets them again.
    private List<Control> _signInDrawn = [];
    private Button? _signInCancel;
    private string? _signInDrawnAccount;
    private bool _signInPageFree = true;

    /// <summary>The Sign-in section the core gave <paramref name="entry"/>'s page.</summary>
    private UIElement AccountSignInGroup(AccountEntry entry, AccountSignIn section)
    {
        WireNextcloudRelease();
        var controls = new StackPanel { Spacing = 8 };
        var consent = entry.Kind is AccountKind.Microsoft or AccountKind.Google;
        var all = new List<Control>();
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        _signInCancel = null;
        foreach (var route in section.Routes)
        {
            var again = route is AccountSignInRoute.SignInAgain;
            var button = new Button { Content = AccountUseText.SignInRoute(route) };
            AutomationProperties.SetAutomationId(button, again ? "AccountSignInAgain" : "AccountSwitchToProvider");
            button.Click += (_, _) => _ = consent
                ? SignInAgainFromPageAsync(entry, [])
                : BrowserSignInAsync(entry, switching: !again, nextcloud: entry.Kind == AccountKind.Dav);
            buttons.Children.Add(button);
            all.Add(button);
        }
        if (buttons.Children.Count > 0)
        {
            if (!consent)
            {
                var cancel = new Button { Content = L10n.ActionCancel(), Visibility = Visibility.Collapsed };
                AutomationProperties.SetAutomationId(cancel, "AccountSignInCancel");
                cancel.Click += (_, _) => _model.CancelAccountSignIn();
                buttons.Children.Add(cancel);
                _signInCancel = cancel;
            }
            controls.Children.Add(buttons);
        }
        if (section.Password is { } field)
        {
            controls.Children.Add(PasswordEntry(entry, field));
        }
        if (section.Switches)
        {
            controls.Children.Add(Description(L10n.SettingsAccountSwitchDescription()));
        }
        _signInDrawn = all;
        _signInDrawnAccount = entry.Id;
        _signInPageFree = section.Control.Enabled;
        ShowSignInProgress(entry.Id);
        return Group(
            L10n.SettingsAccountSigninHeading(),
            AccountUseText.SignInLine(section.Line, AccountNameText.Of(entry)),
            controls);
    }

    // A Nextcloud grant the page held for another try becomes no account once Settings closes.
    private void WireNextcloudRelease()
    {
        if (!_nextcloudClosedWired)
        {
            _nextcloudClosedWired = true;
            Closed += (_, _) => _model.ReleaseAccountNextcloudGrant();
        }
    }

    /// <summary>The password field. One that switches from the provider's sign-in waits behind
    /// "Use a password instead" until it runs; one that replaces a refused password is drawn at
    /// once. It never shows the credential the account holds, and empties as it is sent.</summary>
    private StackPanel PasswordEntry(AccountEntry entry, AccountPasswordField field)
    {
        var panel = new StackPanel { Spacing = 8 };
        var form = new StackPanel { Spacing = 8 };
        var password = new PasswordBox
        {
            Header = field.Kind == AccountSecretKind.PasswordOrToken
                ? L10n.SetupJmapSecretPlaceholder()
                : L10n.SetupFieldPassword(),
        };
        AutomationProperties.SetAutomationId(password, "AccountPassword");
        // Held, too, while the core works on a browser sign-in of the account's.
        var state = field.Save with { Enabled = field.Save.Enabled && _signInProgress.ControlsEnabled(entry.Id) };
        ControlComponents.HoldFields(state, password);
        var save = ControlComponents.FormButton(
            field.Replaces ? L10n.ActionSave() : L10n.SettingsAccountSwitchPasswordSave(),
            state, "AccountPasswordSave", AccentButton(), () =>
            {
                var typed = password.Password;
                if (typed.Length == 0)
                {
                    return;
                }
                // The password replaces a sign-in still out in the browser for this account.
                if (_signInProgress.IsOut(entry.Id))
                {
                    _model.CancelAccountSignIn();
                }
                EndUnreachedQuestion();
                password.Password = string.Empty;
                _model.ChangeAccount(new AccountPageIntent.UsePassword(entry.Id, new TypedPassword(typed)));
            });
        save.Ready(string.Empty);
        password.PasswordChanged += (_, _) => save.Ready(password.Password.Length == 0 ? string.Empty : null);
        form.Children.Add(password);
        form.Children.Add(save.Element);
        if (field.Replaces || field.Save.Running)
        {
            panel.Children.Add(form);
            return panel;
        }
        form.Visibility = Visibility.Collapsed;
        StackPanel? reveal = null;
        reveal = ControlComponents.Button(
            L10n.SettingsAccountSwitchToPassword(),
            state with { Running = false },
            "AccountSwitchToPassword",
            () =>
            {
                reveal!.Visibility = Visibility.Collapsed;
                form.Visibility = Visibility.Visible;
                password.Focus(FocusState.Programmatic);
            });
        panel.Children.Add(reveal);
        panel.Children.Add(form);
        return panel;
    }

    /// <summary>Signs the account in through its provider in the browser, again or in place of
    /// its password, and says how it ended; with Nextcloud for a calendar and contacts account at
    /// a Nextcloud server (<paramref name="nextcloud"/>). A switch that would cost the calendar or
    /// contacts asks first (docs/accounts.md rule 18). A cancelled sign-in says nothing.</summary>
    private async Task BrowserSignInAsync(AccountEntry entry, bool switching, bool nextcloud = false)
    {
        // A press while an earlier sign-in is out supersedes it, and answers its question, if it
        // asked one, as Cancel.
        EndUnreachedQuestion();
        var attempt = _signInProgress.Start(entry.Id);
        ShowSignInProgress(entry.Id);
        SayOnAccounts(null);
        void Step(SignInStep step)
        {
            _signInProgress.Step(attempt, step);
            ShowSignInProgress(entry.Id);
            SaySignInStep(entry, step);
        }
        var ended = nextcloud
            ? await _model.AccountNextcloudSignInAsync(entry.Id, Step)
            : await _model.AccountSignInAsync(entry.Id, AskUnreachedAsync, Step);
        if (!_signInProgress.End(attempt))
        {
            // Superseded: the newer attempt says how it ends.
            return;
        }
        EndUnreachedQuestion();
        ShowSignInProgress(entry.Id);
        NotifyAccounts(ended switch
        {
            { SignedIn: true } => new AccountNotice(
                InfoBarSeverity.Success,
                switching ? L10n.SettingsAccountSwitchedToProvider() : L10n.SettingsAccountSignedIn(),
                string.Empty),
            { Error: { } error } => new AccountNotice(
                InfoBarSeverity.Error,
                switching ? L10n.SettingsAccountSwitchFailed() : L10n.SettingsAccountSigninFailed(),
                error),
            _ => null,
        });
    }

    /// <summary>Says where a browser sign-in is: waiting on the browser while it is out, then
    /// connecting while the code is exchanged and the account connects, which takes seconds when a
    /// calendar moves to an account of its own. Nothing is said before the browser opens:
    /// discovery and any question come first, and a line asking for the browser while nothing is
    /// in it is a dead end.</summary>
    private void SaySignInStep(AccountEntry entry, SignInStep step)
    {
        if (step == SignInStep.Browser)
        {
            SayOnAccounts(new AccountNotice(
                InfoBarSeverity.Informational, string.Empty, L10n.SettingsAccountSigninWaiting(AccountNameText.Of(entry))));
        }
        else if (step == SignInStep.Calling && _accountNotice is { Severity: InfoBarSeverity.Informational })
        {
            SayOnAccounts(new AccountNotice(InfoBarSeverity.Informational, string.Empty, L10n.StatusConnecting()));
        }
    }

    /// <summary>Draws where <paramref name="accountId"/>'s sign-in is onto the section drawn
    /// last, if that section is its.</summary>
    private void ShowSignInProgress(string accountId)
    {
        if (_signInDrawnAccount != accountId)
        {
            return;
        }
        var enabled = _signInPageFree && _signInProgress.ControlsEnabled(accountId);
        foreach (var control in _signInDrawn)
        {
            control.IsEnabled = enabled;
        }
        if (_signInCancel is { } cancel)
        {
            cancel.Visibility = _signInProgress.ShowsCancel(accountId) ? Visibility.Visible : Visibility.Collapsed;
        }
    }
}
