// Settings → Notifications (docs/settings.md, Notifications): whether new mail raises a desktop toast.
// One switch, the same one Linux, Android and iOS draw, over the same catalog copy, drawn from the
// core's NotificationsPage. Off, a pass returns no new mail and still marks it seen, so turning
// notifications on again never floods (docs/background-sync.md).
//
// Windows has its own answer beside ours (Settings → System → Notifications), which is why this
// switch never claims to be the only one. It follows Settings signals while the category is open.
//
// A separate partial to keep SettingsDialog.cs under the 500-line limit.

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The switch while the category is drawn, which a signal moves in place.
    private ToggleSwitch? _newMailToggle;

    private UIElement BuildNotifications()
    {
        var toggle = new ToggleSwitch { Header = L10n.SettingsNotificationsHeading() };
        // A ToggleSwitch takes its automation Name from its Header, which the group heading above
        // it carries too, so a suite matching on the name alone can bind to the inert TextBlock
        // and pass for a switch that was never built (uia.ps1 trap 2).
        AutomationProperties.SetAutomationId(toggle, "NotificationsToggle");
        toggle.Toggled += (_, _) =>
        {
            if (!_rebuilding)
            {
                _model.SetNewMailNotifications(toggle.IsOn);
            }
        };
        _newMailToggle = toggle;
        RefreshNotifications();

        return Group(
            L10n.SettingsNotificationsHeading(),
            L10n.SettingsNotificationsDescription(),
            toggle);
    }

    // The core's choice, and no switch to move before the core is up.
    private void RefreshNotifications()
    {
        if (_newMailToggle is not { } toggle)
        {
            return;
        }
        var page = _model.Notifications;
        toggle.IsEnabled = page is not null;
        if (page is not null && toggle.IsOn != page.NewMail)
        {
            Quietly(() => toggle.IsOn = page.NewMail);
        }
    }
}
