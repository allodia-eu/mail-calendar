// The "your name" field on an account's settings card: what recipients see beside the address on
// the mail this account sends (docs/sending.md).
//
// Its own partial so SettingsDialog.Accounts.cs stays clear of the 500-line limit, and because
// this is the one control on the card whose effect a stranger can see; everything else there is
// about how much of the account this device keeps.

using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>
    /// The name field for one account, or the note that the name is not this person's to change.
    /// </summary>
    /// <remarks>
    /// The edit is sent on <em>losing focus</em>, not on every keystroke: a per-keystroke write
    /// would push a half-typed name to the provider and re-signal the settings surface under the
    /// cursor. Nothing changed means nothing written, so tabbing through the card is free.
    /// </remarks>
    private UIElement BuildSenderName(AccountSyncChoice account)
    {
        var panel = new StackPanel { Spacing = 4 };
        panel.Children.Add(new TextBlock { Text = L10n.SettingsSenderNameHeading() });
        if (!account.SenderNameEditable)
        {
            // The name is the organisation's. Show it, and say why there is no field, rather
            // than a disabled box that reads as a bug.
            panel.Children.Add(new TextBlock
            {
                Text = string.IsNullOrEmpty(account.SenderName) ? account.Email : account.SenderName,
                TextWrapping = TextWrapping.Wrap,
            });
            panel.Children.Add(Description(L10n.SettingsSenderNameManaged()));
            return panel;
        }
        panel.Children.Add(Description(L10n.SettingsSenderNameDescription()));
        var box = new TextBox
        {
            Text = account.SenderName,
            MinWidth = 260,
            HorizontalAlignment = HorizontalAlignment.Left,
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(
            box, L10n.SettingsSenderNameHeading());
        box.LostFocus += (_, _) =>
        {
            if (box.Text == account.SenderName)
            {
                return;
            }
            _model.SetAccountSenderNameChoice(account.AccountId, box.Text);
        };
        panel.Children.Add(box);
        return panel;
    }
}
