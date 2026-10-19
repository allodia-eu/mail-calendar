// An account's page, how a standards or JMAP account signs in (docs/accounts.md rule 17): a
// password, or through its provider, and the switch to the other where its server takes it, or
// signing in again from its stored grant. Microsoft and Google keep "Sign in again" through their
// own consent (AccountUses).
//
// What the server takes is a blocking question, so the section draws its heading and status line
// at once and its buttons once the core answers. The answer is kept for the page's visit and keyed
// by how the account signs in, so a redraw does not ask again and a switch does.
//
// A sign-in waiting on the browser leaves the buttons live, so a press supersedes it, and draws a
// Cancel that retires it (AccountSignInProgress); only a blocking core call disables them.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // What each account's server takes, by account and current way of signing in.
    private readonly Dictionary<string, AccountSignInOptions> _signInOptions = new();

    // The section waiting on each question still out: the latest drawing of it, which the answer
    // fills, so a redraw in the meantime neither asks twice nor loses the answer.
    private readonly Dictionary<string, (StackPanel Controls, AccountEntry Entry)> _signInWaiting = new();

    // Where the page's sign-in is, which every drawing of the section reads.
    private readonly AccountSignInProgress _signInProgress = new();

    // The controls and the Cancel of the section drawn last, and whose, which each step of the
    // sign-in sets again.
    private List<Control> _signInDrawn = [];
    private Button? _signInCancel;
    private string? _signInDrawnAccount;

    private static readonly AccountSignInOptions NothingOffered = new(false, null, false);

    /// <summary>The Sign-in section of a standards or JMAP account's page.</summary>
    private UIElement AccountSignInGroup(AccountEntry entry)
    {
        var status = entry.SignIn switch
        {
            SignInMethod.Provider => L10n.SettingsAccountSignsInProvider(),
            _ => L10n.SettingsAccountSignsInPassword(),
        };
        var controls = new StackPanel { Spacing = 8 };
        var key = $"{entry.Id}\n{entry.SignIn}";
        if (_signInOptions.TryGetValue(key, out var options))
        {
            FillSignIn(controls, entry, options);
        }
        else
        {
            var asking = _signInWaiting.ContainsKey(key);
            _signInWaiting[key] = (controls, entry);
            if (!asking)
            {
                _ = AskSignInOptionsAsync(entry.Id, key);
            }
        }
        return Group(L10n.SettingsAccountSigninHeading(), status, controls);
    }

    private async Task AskSignInOptionsAsync(string accountId, string key)
    {
        var options = await _model.AccountSignInOptionsAsync(accountId) ?? NothingOffered;
        _signInOptions[key] = options;
        // Filling a section a redraw has since replaced is harmless: it is no longer on screen.
        if (_signInWaiting.Remove(key, out var waiting))
        {
            FillSignIn(waiting.Controls, waiting.Entry, options);
        }
    }

    /// <summary>The section's buttons, once the server has said what it takes.</summary>
    private void FillSignIn(StackPanel controls, AccountEntry entry, AccountSignInOptions options)
    {
        if (AccountSettingsRules.SignIn(entry, options) is not { } section)
        {
            return;
        }
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var all = new List<Control>();
        _signInCancel = null;
        if (section.SignInAgain)
        {
            var again = new Button { Content = L10n.SigninExpiredAction() };
            AutomationProperties.SetAutomationId(again, "AccountSignInAgain");
            again.Click += (_, _) => _ = BrowserSignInAsync(entry, switching: false);
            buttons.Children.Add(again);
            all.Add(again);
        }
        if (section.SwitchToProvider)
        {
            var label = section.ProviderLabel is { } provider
                ? L10n.SettingsAccountSwitchToProviderNamed(provider)
                : L10n.SettingsAccountSwitchToProvider();
            var toProvider = new Button { Content = label };
            AutomationProperties.SetAutomationId(toProvider, "AccountSwitchToProvider");
            toProvider.Click += (_, _) => _ = BrowserSignInAsync(entry, switching: true);
            buttons.Children.Add(toProvider);
            all.Add(toProvider);
        }
        if (buttons.Children.Count > 0)
        {
            var cancel = new Button { Content = L10n.ActionCancel(), Visibility = Visibility.Collapsed };
            AutomationProperties.SetAutomationId(cancel, "AccountSignInCancel");
            cancel.Click += (_, _) => _model.CancelAccountSignIn();
            buttons.Children.Add(cancel);
            controls.Children.Add(buttons);
            _signInCancel = cancel;
        }
        if (section.SwitchToPassword)
        {
            controls.Children.Add(PasswordSwitch(entry, all));
        }
        if (section.OffersSwitch)
        {
            controls.Children.Add(Description(L10n.SettingsAccountSwitchDescription()));
        }
        _signInDrawn = all;
        _signInDrawnAccount = entry.Id;
        ShowSignInProgress(entry.Id);
    }

    /// <summary>"Use a password instead", which opens the field it is typed into. The field never
    /// shows the credential the account holds.</summary>
    private StackPanel PasswordSwitch(AccountEntry entry, List<Control> all)
    {
        var panel = new StackPanel { Spacing = 8 };
        var reveal = new Button { Content = L10n.SettingsAccountSwitchToPassword() };
        AutomationProperties.SetAutomationId(reveal, "AccountSwitchToPassword");
        var form = new StackPanel { Spacing = 8, Visibility = Visibility.Collapsed };
        var password = new PasswordBox { Header = L10n.SetupFieldPassword() };
        AutomationProperties.SetAutomationId(password, "AccountSwitchPassword");
        var save = new Button { Content = L10n.SettingsAccountSwitchPasswordSave(), Style = AccentButton() };
        AutomationProperties.SetAutomationId(save, "AccountSwitchPasswordSave");
        form.Children.Add(password);
        form.Children.Add(save);
        panel.Children.Add(reveal);
        panel.Children.Add(form);
        all.Add(reveal);
        all.Add(save);
        reveal.Click += (_, _) =>
        {
            reveal.Visibility = Visibility.Collapsed;
            form.Visibility = Visibility.Visible;
            password.Focus(FocusState.Programmatic);
        };
        save.Click += async (_, _) =>
        {
            if (password.Password.Length == 0)
            {
                return;
            }
            // The password replaces a sign-in still out in the browser for this account.
            if (_signInProgress.IsOut(entry.Id))
            {
                _model.CancelAccountSignIn();
            }
            EndUnreachedQuestion();
            var attempt = _signInProgress.Start(entry.Id);
            ShowSignInProgress(entry.Id);
            var error = await _model.SwitchAccountToPasswordAsync(entry.Id, password.Password);
            _signInProgress.End(attempt);
            ShowSignInProgress(entry.Id);
            if (error is null)
            {
                password.Password = string.Empty;
                NotifyAccounts(new AccountNotice(
                    InfoBarSeverity.Success, L10n.SettingsAccountSwitchedToPassword(), string.Empty));
            }
            else
            {
                // The field keeps what was typed, so a mistyped password is corrected, not retyped.
                SayOnAccounts(new AccountNotice(InfoBarSeverity.Error, L10n.SettingsAccountSwitchFailed(), error));
            }
        };
        return panel;
    }

    /// <summary>Signs the account in through its provider in the browser, again or in place of
    /// its password, and says how it ended. A switch that would cost the calendar or contacts asks
    /// first (docs/accounts.md rule 18). A cancelled sign-in says nothing.</summary>
    private async Task BrowserSignInAsync(AccountEntry entry, bool switching)
    {
        // A press while an earlier sign-in is out supersedes it, and answers its question, if it
        // asked one, as Cancel.
        EndUnreachedQuestion();
        var attempt = _signInProgress.Start(entry.Id);
        ShowSignInProgress(entry.Id);
        SayOnAccounts(null);
        var ended = await _model.AccountSignInAsync(
            entry.Id,
            AskUnreachedAsync,
            step =>
            {
                _signInProgress.Step(attempt, step);
                ShowSignInProgress(entry.Id);
                SaySignInStep(entry, step);
            });
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
        var enabled = _signInProgress.ControlsEnabled(accountId);
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
