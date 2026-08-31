// Settings → Privacy: the usage-statistics opt-in the welcome screen asked about, withdrawable
// here in one click. GDPR Art. 7(3): withdrawal must be as easy as giving, which is why this is
// the same single switch, not a buried confirmation flow. Turning it off deletes the install id
// locally and asks the backend to erase everything held under it (Art. 17).
//
// Its twins are macOS's AnalyticsSettings.swift and Android's AnalyticsConsentUi.kt, keep the
// wording and the rules in step; docs/analytics.md is the contract.
//
// A separate partial to keep SettingsDialog.cs under the 500-line limit.

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private UIElement BuildPrivacy()
    {
        var toggle = new ToggleSwitch
        {
            Header = L10n.SettingsAnalyticsToggle(),
            IsOn = _model.AnalyticsEnabled,
        };
        // Attached after IsOn is set, so seeding the initial state does not itself look like the
        // user flipping the switch, the same trap the Radio() helper guards against.
        toggle.Toggled += (_, _) =>
        {
            if (!_rebuilding)
            {
                _model.SetAnalyticsConsent(toggle.IsOn);
            }
        };

        var payload = new TextBlock
        {
            FontFamily = new FontFamily("Consolas"),
            IsTextSelectionEnabled = true,
            TextWrapping = TextWrapping.NoWrap,
        };
        // Monospaced and scrolled sideways rather than wrapped: a re-flowed payload is a paraphrase,
        // and the point of showing it is that it is not one. An inline reveal, because we are
        // already inside a ContentDialog and WinUI does not allow a nested one.
        var panel = new ScrollViewer
        {
            Visibility = Visibility.Collapsed,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
            HorizontalScrollMode = ScrollMode.Enabled,
            Padding = new Thickness(12),
            CornerRadius = new CornerRadius(4),
            Content = payload,
        };

        var reveal = new HyperlinkButton { Content = L10n.WelcomeAnalyticsPreview() };
        reveal.Click += (_, _) =>
        {
            if (panel.Visibility == Visibility.Visible)
            {
                panel.Visibility = Visibility.Collapsed;
                return;
            }
            // Pulled fresh each time: opting in mints the install id, so the payload the user sees
            // here is the one their next event will actually carry.
            payload.Text = _model.AnalyticsPayloadPreview();
            panel.Visibility = Visibility.Visible;
        };

        var content = new StackPanel { Spacing = 8 };
        content.Children.Add(toggle);
        content.Children.Add(reveal);
        content.Children.Add(panel);

        return Group(L10n.SettingsAnalyticsHeading(), L10n.SettingsAnalyticsDescription(), content);
    }
}
