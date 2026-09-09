// Settings → Notifications (docs/settings.md slot 7): whether new mail raises a desktop toast.
// One switch, the same one Linux, Android and iOS draw, over the same catalog copy.
//
// The choice is the HOST's, not the core's: what may appear on this desktop is a question about
// this machine, and Windows has its own answer beside ours (Settings → System → Notifications),
// which is why this switch never claims to be the only one. It gates posting alone; the scan
// behind it keeps running, so turning notifications off and on again never floods
// (docs/background-sync.md).
//
// A separate partial to keep SettingsDialog.cs under the 500-line limit.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private UIElement BuildNotifications()
    {
        var toggle = new ToggleSwitch
        {
            Header = L10n.SettingsNotificationsHeading(),
            IsOn = NotificationPrefs.Enabled(),
        };
        // A ToggleSwitch takes its automation Name from its Header, which the group heading above
        // it carries too, so a suite matching on the name alone can bind to the inert TextBlock
        // and pass for a switch that was never built (uia.ps1 trap 2).
        AutomationProperties.SetAutomationId(toggle, "NotificationsToggle");
        // Attached after IsOn is set, so seeding the initial state does not itself read as the
        // user flipping the switch (the same trap Privacy's toggle guards against).
        toggle.Toggled += (_, _) =>
        {
            if (!_rebuilding)
            {
                NotificationPrefs.SetEnabled(toggle.IsOn);
            }
        };

        return Group(
            L10n.SettingsNotificationsHeading(),
            L10n.SettingsNotificationsDescription(),
            toggle);
    }
}
