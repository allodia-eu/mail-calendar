// Settings → Composing → Default send account, as the core offers it (SendAccountSetting): no
// account, the one address, or a choice over every account used for mail on the one a new message
// is written from. Which account that is, is the core's; nothing here decides it.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The group's content, drawn again when the accounts on offer change. A change of default
    // alone moves the picker in place, so the choice just made keeps its focus.
    private StackPanel? _sendAccount;
    private ComboBox? _sendAccountBox;
    private string? _sendAccountDrawn;

    private StackPanel SendAccountControl()
    {
        _sendAccount = new StackPanel();
        _sendAccountDrawn = null;
        RefreshSendAccount();
        return _sendAccount;
    }

    private void RefreshSendAccount()
    {
        if (_sendAccount is not { } host)
        {
            return;
        }
        var setting = _model.SendAccountSetting;
        var drawn = SendAccountChoices.Fingerprint(setting);
        if (drawn == _sendAccountDrawn)
        {
            if (setting is SendAccountSetting.Choose offered && _sendAccountBox is { } picker)
            {
                SelectSendAccount(picker, offered);
            }
            return;
        }
        _sendAccountDrawn = drawn;
        _sendAccountBox = null;
        host.Children.Clear();
        UIElement content = setting switch
        {
            SendAccountSetting.Choose choose => SendAccountPicker(choose),
            SendAccountSetting.Only only => Description(only.Email),
            _ => Description(L10n.SettingsAccountsEmpty()),
        };
        host.Children.Add(content);
    }

    private ComboBox SendAccountPicker(SendAccountSetting.Choose choose)
    {
        var box = new ComboBox { MinWidth = 260 };
        AutomationProperties.SetName(box, L10n.SettingsSendAccountHeading());
        AutomationProperties.SetAutomationId(box, "SendAccountPicker");
        foreach (var account in choose.Accounts)
        {
            box.Items.Add(new ComboBoxItem { Content = account.Email, Tag = account.Id });
        }
        SelectSendAccount(box, choose);
        box.SelectionChanged += (_, _) =>
        {
            if (!_rebuilding && (box.SelectedItem as ComboBoxItem)?.Tag is string id)
            {
                _model.SetDefaultSendAccount(id);
            }
        };
        _sendAccountBox = box;
        return box;
    }

    /// <summary>Selects the core's account without storing it again: it is what the core already
    /// holds.</summary>
    private void SelectSendAccount(ComboBox box, SendAccountSetting.Choose choose)
    {
        var at = SendAccountChoices.SelectedIndex(choose);
        if (box.SelectedIndex == at)
        {
            return;
        }
        var was = _rebuilding;
        _rebuilding = true;
        try
        {
            box.SelectedIndex = at;
        }
        finally
        {
            _rebuilding = was;
        }
    }
}
