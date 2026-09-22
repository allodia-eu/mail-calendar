// The manual form's server row: the port box and the security picker beside each server name,
// and the one rule that makes them work together (docs/account-autodetect.md). Split from
// AccountSetupView.xaml.cs to keep that file under the 500-line limit.
//
// The trap this file exists to avoid: writing a port into the box raises the same TextChanged the
// user does, so a naive handler reads the app's own fill as the user taking the port over and the
// picker stops working. `_fillingServerFields` is what tells the two apart.

using Allodia.Mailcal;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    private void OnImapSecurityChanged(object sender, SelectionChangedEventArgs e)
    {
        _imap.ChooseSecurity(SelectedSecurity(ImapSecurityPicker));
        ShowPort(ImapPort, _imap);
    }

    private void OnSmtpSecurityChanged(object sender, SelectionChangedEventArgs e)
    {
        _smtp.ChooseSecurity(SelectedSecurity(SmtpSecurityPicker));
        ShowPort(SmtpPort, _smtp);
    }

    private void OnImapPortChanged(object sender, TextChangedEventArgs e)
    {
        if (!_fillingServerFields)
        {
            _imap.TypePort(ImapPort.Text);
        }
    }

    private void OnSmtpPortChanged(object sender, TextChangedEventArgs e)
    {
        if (!_fillingServerFields)
        {
            _smtp.TypePort(SmtpPort.Text);
        }
    }

    private static ConnectionSecurity SelectedSecurity(ComboBox picker) =>
        picker.SelectedIndex == 1 ? ConnectionSecurity.StartTls : ConnectionSecurity.ImplicitTls;

    // Writes a port the code decided, without that write counting as the user typing one.
    private void ShowPort(TextBox box, ManualServerField field)
    {
        _fillingServerFields = true;
        box.Text = field.Port;
        _fillingServerFields = false;
    }

    // Puts a detected route's hosts, ports and security on screen in one go.
    private void ShowServerFields(string imapHost, string smtpHost)
    {
        _fillingServerFields = true;
        ImapHost.Text = imapHost;
        SmtpHost.Text = smtpHost;
        ImapPort.Text = _imap.Port;
        SmtpPort.Text = _smtp.Port;
        ImapSecurityPicker.SelectedIndex = _imap.Security == ConnectionSecurity.StartTls ? 1 : 0;
        SmtpSecurityPicker.SelectedIndex = _smtp.Security == ConnectionSecurity.StartTls ? 1 : 0;
        _fillingServerFields = false;
    }
}
