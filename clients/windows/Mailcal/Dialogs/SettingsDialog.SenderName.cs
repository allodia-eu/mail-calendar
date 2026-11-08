// The "your name" field on an account's page: what recipients see beside the address on the mail
// this account sends (docs/sending.md), as the core's page record says it.

using Allodia.Mailcal.Controls;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>The name field for one account, or the note that the name is not this person's to
    /// change. Drawn directly under the sharing control: the one thing on the page a recipient can
    /// see (docs/settings.md).</summary>
    private UIElement SenderNameGroup(AccountEntry entry, AccountSenderName name)
    {
        var panel = new StackPanel { Spacing = 4 };
        if (!name.Editable)
        {
            // The name is the organisation's. Show it, and say why there is no field, rather
            // than a disabled box that reads as a bug.
            panel.Children.Add(new TextBlock { Text = name.Name, TextWrapping = TextWrapping.Wrap });
            panel.Children.Add(Description(L10n.SettingsSenderNameManaged()));
        }
        else
        {
            panel.Children.Add(ControlComponents.TextBox(
                L10n.SettingsSenderNameHeading(), name.Name, name.Control, "AccountSenderName",
                typed => _model.ChangeAccount(new AccountPageIntent.SetSenderName(entry.Id, typed))));
        }
        return Group(L10n.SettingsSenderNameHeading(), L10n.SettingsSenderNameDescription(), panel);
    }
}
