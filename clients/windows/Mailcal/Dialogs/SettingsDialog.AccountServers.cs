// An account's page, its last questions: the servers and password of an account that signs in
// with a password to servers of its own (docs/accounts.md rule 14), and its removal, which names
// the accounts that lose their link to it (rule 12).
//
// Saving tries the servers before keeping them (SaveServers), so a typo never costs a working
// account, and an expired password is replaced here too.

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
    /// <summary>The servers form, drawn from the page's <paramref name="servers"/>: the edit being
    /// saved while a save runs, the one the last save refused so one field is corrected rather than
    /// all retyped, and otherwise what is stored. The page is not drawn again while the form holds
    /// edits Save has not sent.</summary>
    private UIElement ServersGroup(AccountEntry entry, AccountServers servers, bool expired)
    {
        var form = servers.Form;
        var stack = new StackPanel { Spacing = 8 };
        var imap = form.ImapHost is { } imapHost
            ? ServerRows(stack, L10n.SettingsAccountFieldImap(), imapHost, form.ImapSecurity, MailServerKind.Imap)
            : null;
        var smtp = form.SmtpHost is { } smtpHost
            ? ServerRows(stack, L10n.SetupFieldSmtp(), smtpHost, form.SmtpSecurity, MailServerKind.Smtp)
            : null;
        var caldav = form.CaldavUrl is { } caldavUrl
            ? Field(stack, L10n.SetupFieldCaldav(), caldavUrl)
            : null;
        var carddav = form.CarddavUrl is { } carddavUrl
            ? Field(stack, L10n.SettingsAccountFieldCarddav(), carddavUrl)
            : null;
        var login = Field(stack, L10n.SettingsAccountFieldLogin(), form.Username);
        var password = new PasswordBox
        {
            Header = L10n.SettingsAccountFieldNewPassword(),
            Password = form.Password ?? string.Empty,
        };
        AutomationProperties.SetAutomationId(password, "AccountNewPassword");
        stack.Children.Add(password);
        // Held with Save, so nothing typed during a save is drawn over when it ends.
        ControlComponents.HoldFields(
            servers.Save with { Running = false },
            imap?.Host, imap?.Port, imap?.Picker, smtp?.Host, smtp?.Port, smtp?.Picker,
            caldav, carddav, login, password);

        AccountEndpoints Read() => AccountEndpointEdit.Edited(form, new EndpointFields(
            imap?.Address(), imap?.Security(),
            smtp?.Address(), smtp?.Security(),
            caldav?.Text.Trim(), carddav?.Text.Trim(),
            login.Text.Trim(), password.Password));
        var drawn = Read();
        var state = ControlComponents.Form(stack);
        var save = ControlComponents.FormButton(
            L10n.ActionSave(), servers.Save, "AccountServersSave", AccentButton(), () =>
            {
                var endpoints = Read();
                drawn = endpoints;
                state.Unsent = false;
                _model.ChangeAccount(new AccountPageIntent.SaveServers(entry.Id, endpoints));
            });
        void Follow()
        {
            var now = Read();
            state.Unsent = now != drawn;
            save.Ready(_model.EndpointsGap(entry.Id, now) is { } gap ? AccountUseText.EndpointsGap(gap) : null);
        }
        foreach (var box in new[] { imap?.Host, imap?.Port, smtp?.Host, smtp?.Port, caldav, carddav, login })
        {
            if (box is not null)
            {
                box.TextChanged += (_, _) => Follow();
            }
        }
        foreach (var picker in new[] { imap?.Picker, smtp?.Picker })
        {
            if (picker is not null)
            {
                picker.SelectionChanged += (_, _) => Follow();
            }
        }
        password.PasswordChanged += (_, _) => Follow();
        Follow();
        stack.Children.Add(save.Element);
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
        var asking = $"remove:{entry.Id}";
        // The page runs the removal and goes back to the list once the account is gone; an erase
        // that failed is said there (MailboxModel.RemovalFailed).
        var confirm = new ConfirmPanel(
            L10n.RemoveAccountTitle(), message, L10n.ActionRemove(), "AccountRemove", yes =>
            {
                _confirmOpen = null;
                remove!.Visibility = Visibility.Visible;
                if (yes)
                {
                    _model.ChangeAccount(new AccountPageIntent.Remove(entry.Id));
                }
            });
        remove = ControlComponents.Button(L10n.ActionRemoveAccount(), control, "AccountRemove", () =>
        {
            _confirmOpen = asking;
            remove!.Visibility = Visibility.Collapsed;
            confirm.Show();
        });
        if (_confirmOpen == asking)
        {
            remove.Visibility = Visibility.Collapsed;
            confirm.Show();
        }
        confirm.Enabled = control.Enabled;
        stack.Children.Add(remove);
        stack.Children.Add(confirm.Element);
        return stack;
    }
}
