// Settings → Reading → the message background in a dark app: the same preference as the reading
// view's sun/moon button (docs/reading-dark-mode.md). Its own partial to keep SettingsDialog.cs
// under the 500-line limit.

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private UIElement DarkMessagesToggle()
    {
        var toggle = new ToggleSwitch
        {
            Header = L10n.SettingsDarkMessagesToggle(),
            IsOn = _model.DarkMessageBackground,
        };
        AutomationProperties.SetAutomationId(toggle, "SettingsDarkMessages");
        AutomationProperties.SetName(toggle, L10n.SettingsDarkMessagesToggle());
        toggle.Toggled += (_, _) =>
        {
            if (!_rebuilding)
            {
                _model.SetDarkMessageBackground(toggle.IsOn);
            }
        };
        return toggle;
    }
}
