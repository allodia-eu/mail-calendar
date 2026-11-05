// Settings → Videoconferencing (docs/settings.md slot 4): the person's Jitsi server, the one field
// the category holds (docs/conferencing.md, rule 13). Empty until they fill it, and checked by the
// core before it is stored, so an address it refuses is said here and nothing changes. Once one is
// stored, every calendar whose provider writes a meeting offers Jitsi in the editor.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Windows.System;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private UIElement BuildVideoconferencing()
    {
        var box = new TextBox
        {
            Text = _model.JitsiServer() ?? string.Empty,
            PlaceholderText = "https://meet.example.org",
            MinWidth = 320,
            HorizontalAlignment = HorizontalAlignment.Left,
            IsSpellCheckEnabled = false,
        };
        AutomationProperties.SetName(box, L10n.SettingsJitsiServer());
        AutomationProperties.SetAutomationId(box, "JitsiServerBox");
        var refused = new TextBlock
        {
            Text = L10n.SettingsJitsiServerInvalid(),
            TextWrapping = TextWrapping.Wrap,
            Foreground = _brushes.Of(ThemePalette.Critical),
            Visibility = Visibility.Collapsed,
        };
        AutomationProperties.SetAutomationId(refused, "JitsiServerRefused");

        // Sent on Enter or on losing focus, never per keystroke: a half-typed address is one the
        // core refuses. The stored form (its trailing '/' dropped) is what the field shows after.
        void Store()
        {
            if (box.Text == (_model.JitsiServer() ?? string.Empty))
            {
                return;
            }
            if (_model.SetJitsiServer(box.Text))
            {
                box.Text = _model.JitsiServer() ?? string.Empty;
            }
            else
            {
                refused.Visibility = Visibility.Visible;
            }
        }
        box.TextChanged += (_, _) => refused.Visibility = Visibility.Collapsed;
        box.LostFocus += (_, _) => Store();
        box.KeyDown += (_, args) =>
        {
            if (args.Key == VirtualKey.Enter)
            {
                Store();
                args.Handled = true;
            }
        };

        var field = new StackPanel { Spacing = 4, Children = { box, refused } };
        return Group(L10n.SettingsJitsiServer(), L10n.SettingsJitsiServerHint(), field);
    }
}
