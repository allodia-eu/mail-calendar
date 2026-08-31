// Settings → About: which release this is, where to ask for help, and whose work it is built on.
// The content is the core's (AboutInfo) so every client says the same thing, a support answer
// that names a version has to name the same version everywhere; only the labels are the catalog's.
//
// Its twins are Linux's settings/about.rs, Android's SettingsAbout.kt and Apple's About detail.
//
// A separate partial to keep SettingsDialog.cs under the 500-line limit.

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private UIElement BuildAbout()
    {
        var about = MailcalBindingsMethods.AboutInfo(AboutPlatform.Windows);
        var panel = new StackPanel { Spacing = 24 };

        panel.Children.Add(Group(
            L10n.AppTitle(),
            L10n.AboutVersion(about.Version),
            new StackPanel()));

        var support = new StackPanel { Spacing = 8 };
        support.Children.Add(new TextBlock { Text = about.SupportUrl, IsTextSelectionEnabled = true });
        var open = new Button { Content = L10n.AboutSupportAction() };
        // The forum opens in the user's browser: this app's only WebView2 is the locked-down
        // reading island (docs/rendering-security.md), which is not a browser.
        open.Click += async (_, _) =>
            await Windows.System.Launcher.LaunchUriAsync(new System.Uri(about.SupportUrl));
        support.Children.Add(open);
        panel.Children.Add(Group(
            L10n.AboutSupportHeading(), L10n.AboutSupportDescription(), support));

        var attributions = new StackPanel { Spacing = 10 };
        foreach (var item in about.Attributions)
        {
            var entry = new StackPanel();
            entry.Children.Add(new TextBlock { Text = item.Name, IsTextSelectionEnabled = true });
            entry.Children.Add(new TextBlock
            {
                Text = item.License,
                Opacity = 0.7,
                IsTextSelectionEnabled = true,
            });
            attributions.Children.Add(entry);
        }
        panel.Children.Add(Group(
            L10n.AboutAttributionsHeading(), L10n.AboutAttributionsDescription(), attributions));

        return panel;
    }
}
