// An account's page, its last questions: the servers and password of an account that signs in
// with a password to servers of its own (docs/accounts.md rule 14), and its removal, which names
// the accounts that lose their link to it (rule 12).
//
// Saving tries the servers before keeping them (update_account_endpoints), so a typo never costs
// a working account, and an expired password is replaced here too.

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
    private UIElement ServersGroup(AccountEntry entry, AccountEndpoints endpoints, bool expired)
    {
        var stack = new StackPanel { Spacing = 8 };
        var imap = endpoints.ImapHost is { } imapHost
            ? ServerRows(stack, L10n.SettingsAccountFieldImap(), imapHost, endpoints.ImapSecurity, MailServerKind.Imap)
            : null;
        var smtp = endpoints.SmtpHost is { } smtpHost
            ? ServerRows(stack, L10n.SetupFieldSmtp(), smtpHost, endpoints.SmtpSecurity, MailServerKind.Smtp)
            : null;
        var caldav = endpoints.CaldavUrl is { } caldavUrl
            ? Field(stack, L10n.SetupFieldCaldav(), caldavUrl)
            : null;
        var carddav = endpoints.CarddavUrl is { } carddavUrl
            ? Field(stack, L10n.SettingsAccountFieldCarddav(), carddavUrl)
            : null;
        var login = Field(stack, L10n.SettingsAccountFieldLogin(), endpoints.Username);
        var password = new PasswordBox { Header = L10n.SettingsAccountFieldNewPassword() };
        AutomationProperties.SetAutomationId(password, "AccountNewPassword");
        stack.Children.Add(password);
        var save = new Button { Content = L10n.ActionSave(), Style = AccentButton() };
        AutomationProperties.SetAutomationId(save, "AccountServersSave");
        save.Click += async (_, _) =>
        {
            var fields = new EndpointFields(
                imap?.Address(), imap?.Security(),
                smtp?.Address(), smtp?.Security(),
                caldav?.Text.Trim(), carddav?.Text.Trim(),
                login.Text.Trim(), password.Password);
            SayOnAccounts(new AccountNotice(
                InfoBarSeverity.Informational, string.Empty, L10n.SettingsAccountServersTrying()));
            save.IsEnabled = false;
            var error = await _model.SaveAccountEndpointsAsync(
                entry.Id, AccountEndpointEdit.Edited(endpoints, fields));
            save.IsEnabled = true;
            if (error is null)
            {
                NotifyAccounts(new AccountNotice(
                    InfoBarSeverity.Success, L10n.SettingsAccountServersSaved(), string.Empty));
            }
            else
            {
                // The form stays as the person typed it, the password included, so they correct
                // one field rather than retype them all.
                SayOnAccounts(new AccountNotice(InfoBarSeverity.Error, L10n.SettingsAccountServersFailed(), error));
            }
        };
        stack.Children.Add(save);
        var description = expired
            ? L10n.SigninExpiredPrompt(AccountNameText.Of(entry))
            : L10n.SettingsAccountServersDescription();
        return Group(L10n.SettingsAccountServersHeading(), description, stack);
    }

    private static TextBox Field(StackPanel stack, string header, string text)
    {
        var field = new TextBox { Header = header, Text = text };
        stack.Children.Add(field);
        return field;
    }

    /// <summary>
    /// One server's host, port and security, on one row as the setup form lays them out, and with
    /// its rule: the port follows the security until the person types one of their own
    /// (<see cref="ManualServerField"/>).
    /// </summary>
    private static EditedServer ServerRows(
        StackPanel stack, string header, string address, ConnectionSecurity security, MailServerKind kind)
    {
        ushort Standard(ConnectionSecurity chosen) => MailcalBindingsMethods.StandardPort(kind, chosen);
        var field = ManualServerField.For(kind);
        field.AdoptDetected(address, security);
        var hostText = ManualServerField.SplitHost(address).Host;
        var portText = field.Port;
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var host = new TextBox { Header = header, Text = hostText };
        var port = new TextBox { Header = L10n.SetupFieldPort(), Text = portText, Width = 88 };
        var picker = new ComboBox { Header = L10n.SetupFieldSecurity(), Width = 132 };
        picker.Items.Add(L10n.SetupSecurityImplicitTls());
        picker.Items.Add(L10n.SetupSecurityStarttls());
        picker.SelectedIndex = security == ConnectionSecurity.StartTls ? 1 : 0;
        Grid.SetColumn(port, 1);
        Grid.SetColumn(picker, 2);
        grid.Children.Add(host);
        grid.Children.Add(port);
        grid.Children.Add(picker);
        stack.Children.Add(grid);
        var server = new EditedServer(host, port, picker, Standard);
        // TypePort tells the form's own write from typing by value, so writing the followed port
        // back raises nothing that takes the port over.
        port.TextChanged += (_, _) => field.TypePort(port.Text);
        picker.SelectionChanged += (_, _) =>
        {
            field.ChooseSecurity(server.Security());
            port.Text = field.Port;
        };
        return server;
    }

    private sealed record EditedServer(
        TextBox Host, TextBox Port, ComboBox Picker, Func<ConnectionSecurity, ushort> Standard)
    {
        public ConnectionSecurity Security() =>
            Picker.SelectedIndex == 1 ? ConnectionSecurity.StartTls : ConnectionSecurity.ImplicitTls;

        public string Address() => AccountEndpointEdit.Joined(Host.Text, Port.Text, Security(), Standard);
    }

    /// <summary>Removing the account, confirmed inline and naming the accounts that lose their
    /// link to it. Last on the page, below everything the person might adjust.</summary>
    private UIElement RemoveGroup(AccountEntry entry, ControlState control)
    {
        var stack = new StackPanel { Spacing = 8 };
        var message = L10n.RemoveAccountMessage(AccountNameText.Of(entry));
        if (AccountSettingsRules.Unlinked(entry, AccountNameText.Words) is { } unlinked)
        {
            message += " " + L10n.RemoveAccountUnlinks(unlinked);
        }
        StackPanel? remove = null;
        // The page runs the removal and goes back to the list once the account is gone; an erase
        // that failed is said there (MailboxModel.RemovalFailed).
        var confirm = new ConfirmPanel(
            L10n.RemoveAccountTitle(), message, L10n.ActionRemove(), "AccountRemove", yes =>
            {
                remove!.Visibility = Visibility.Visible;
                if (yes)
                {
                    _model.ChangeAccount(new AccountPageIntent.Remove(entry.Id));
                }
            });
        remove = ControlComponents.Button(L10n.ActionRemoveAccount(), control, "AccountRemove", () =>
        {
            remove!.Visibility = Visibility.Collapsed;
            confirm.Show();
        });
        stack.Children.Add(remove);
        stack.Children.Add(confirm.Element);
        return stack;
    }
}
