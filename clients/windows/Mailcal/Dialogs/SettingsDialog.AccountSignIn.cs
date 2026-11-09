// An account's page, how it signs in (docs/accounts.md rule 17), drawn from the core's record: the
// line saying how it signs in or what is wrong, the ways in through a browser, the browser sign-in
// while one is out, and the field a password is typed into. Which of them it offers, and where a
// browser sign-in is, are the core's (MailboxModel.BrowserSignIn.cs runs the browser half).
//
// A route stays pressable while the browser holds a sign-in, so a press supersedes one abandoned
// there; the section says it waits on the browser and offers a Cancel that ends it.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>The Sign-in section the core gave <paramref name="entry"/>'s page.</summary>
    private UIElement AccountSignInGroup(AccountEntry entry, AccountSignIn section)
    {
        var controls = new StackPanel { Spacing = 8 };
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        foreach (var route in section.Routes)
        {
            buttons.Children.Add(ControlComponents.Button(
                AccountUseText.SignInRoute(route),
                section.Control,
                route is AccountSignInRoute.SignInAgain ? "AccountSignInAgain" : "AccountSwitchToProvider",
                () => _model.SignIn(entry.Id, [])));
        }
        if (buttons.Children.Count > 0)
        {
            controls.Children.Add(buttons);
        }
        if (BrowserWaiting(entry, section.Browser) is { } waiting)
        {
            controls.Children.Add(waiting);
        }
        if (section.Password is { } field)
        {
            controls.Children.Add(PasswordEntry(entry, field));
        }
        if (section.Switches)
        {
            controls.Children.Add(Description(L10n.SettingsAccountSwitchDescription()));
        }
        return Group(
            L10n.SettingsAccountSigninHeading(),
            AccountUseText.SignInLine(section.Line, AccountNameText.Of(entry)),
            controls);
    }

    /// <summary>What the section says while the browser holds <paramref name="browser"/>, with its
    /// Cancel; null at every other step.</summary>
    private StackPanel? BrowserWaiting(AccountEntry entry, BrowserSignIn? browser)
    {
        if (browser is null || BrowserSignInRules.WaitingLine(browser) is not { } line)
        {
            return null;
        }
        var panel = new StackPanel { Spacing = 8 };
        var said = new TextBlock
        {
            Text = line == SignInWaitLine.Nextcloud
                ? L10n.NextcloudSignInWaiting()
                : L10n.SettingsAccountSigninWaiting(AccountNameText.Of(entry)),
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetAutomationId(said, "AccountSignInWaiting");
        AutomationProperties.SetLiveSetting(said, AutomationLiveSetting.Polite);
        panel.Children.Add(said);
        panel.Children.Add(ControlComponents.Button(
            L10n.ActionCancel(), browser.Cancel, "AccountSignInCancel", () => _model.CancelSignIn(browser)));
        return panel;
    }

    /// <summary>The password field. One that switches from the provider's sign-in waits behind
    /// "Use a password instead" until it runs; one that replaces a refused password is drawn at
    /// once. It never shows the credential the account holds, and empties as it is sent; while it
    /// holds text, a redraw waits. The core holds it while a browser sign-in of the account's is
    /// out.</summary>
    private StackPanel PasswordEntry(AccountEntry entry, AccountPasswordField field)
    {
        var panel = new StackPanel { Spacing = 8 };
        var form = new StackPanel { Spacing = 8 };
        var unsent = ControlComponents.Form(form);
        var password = new PasswordBox
        {
            Header = field.Kind == AccountSecretKind.PasswordOrToken
                ? L10n.SetupJmapSecretPlaceholder()
                : L10n.SetupFieldPassword(),
        };
        AutomationProperties.SetAutomationId(password, "AccountPassword");
        var state = field.Save;
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
                _model.ChangeAccount(new AccountPageIntent.UsePassword(entry.Id, new TypedPassword(typed)));
                password.Password = string.Empty;
            });
        save.Ready(string.Empty);
        password.PasswordChanged += (_, _) =>
        {
            unsent.Unsent = password.Password.Length > 0;
            save.Ready(password.Password.Length == 0 ? string.Empty : null);
        };
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
}
