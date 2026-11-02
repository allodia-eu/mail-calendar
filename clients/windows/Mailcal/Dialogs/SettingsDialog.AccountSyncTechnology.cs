// An account's page, which way a Gmail account's mail syncs in this launch (docs/settings.md row
// 9): Optimised, Fallback, Can't connect while its mail cannot be reached, or Checking until the
// first read has settled it. The status never names
// a protocol; the info button opens a flyout with what it means and a link to the help page.

using Allodia.Mailcal.Services;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>The Sync technology row of a Gmail account's page.</summary>
    private static UIElement SyncTechnologyGroup(MailSyncTechnology technology)
    {
        var row = new Grid { ColumnSpacing = 8 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var heading = Heading(L10n.SettingsAccountSyncTechnologyHeading());
        heading.VerticalAlignment = VerticalAlignment.Center;
        row.Children.Add(heading);

        var status = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
            VerticalAlignment = VerticalAlignment.Center,
        };
        var dot = new Ellipse { Width = 8, Height = 8, Fill = SyncTechnologyBrush(technology) };
        AutomationProperties.SetAccessibilityView(dot, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        status.Children.Add(dot);
        var name = new TextBlock { Text = SyncTechnologyLabels.Name(technology) };
        AutomationProperties.SetAutomationId(name, "AccountSyncTechnology");
        status.Children.Add(name);
        Grid.SetColumn(status, 1);
        row.Children.Add(status);

        var explanation = new StackPanel { Spacing = 8, MaxWidth = 320 };
        explanation.Children.Add(new TextBlock
        {
            Text = SyncTechnologyLabels.Detail(technology),
            TextWrapping = TextWrapping.Wrap,
        });
        var learnMore = new HyperlinkButton { Content = L10n.SettingsAccountSyncTechnologyLearnMore() };
        // The help page opens in the user's browser: this app's only WebView2 is the locked-down
        // reading island (docs/rendering-security.md), which is not a browser.
        learnMore.Click += async (_, _) => await Windows.System.Launcher.LaunchUriAsync(
            new System.Uri(MailcalBindingsMethods.HelpPageUrl(HelpPage.OptimiseGmailSync)));
        explanation.Children.Add(learnMore);

        var info = new Button
        {
            Content = new FontIcon { Glyph = "", FontSize = 14 },
            Padding = new Thickness(8),
            Flyout = new Flyout { Content = explanation },
        };
        AutomationProperties.SetName(info, L10n.SettingsAccountSyncTechnologyAbout());
        AutomationProperties.SetAutomationId(info, "AccountSyncTechnologyAbout");
        ToolTipService.SetToolTip(info, L10n.SettingsAccountSyncTechnologyAbout());
        Grid.SetColumn(info, 2);
        row.Children.Add(info);
        return row;
    }

    private static Brush SyncTechnologyBrush(MailSyncTechnology technology) => technology switch
    {
        MailSyncTechnology.Optimised => new SolidColorBrush(Colors.ForestGreen),
        MailSyncTechnology.Fallback => new SolidColorBrush(Colors.DarkOrange),
        MailSyncTechnology.Unreachable => new SolidColorBrush(Colors.Firebrick),
        _ => new SolidColorBrush(Colors.Gray),
    };
}

/// <summary>What each sync technology is called and explained as, apart from the page so it can be
/// tested without WinUI.</summary>
internal static class SyncTechnologyLabels
{
    public static string Name(MailSyncTechnology technology) => technology switch
    {
        MailSyncTechnology.Optimised => L10n.SettingsAccountSyncTechnologyOptimised(),
        MailSyncTechnology.Fallback => L10n.SettingsAccountSyncTechnologyFallback(),
        MailSyncTechnology.Unreachable => L10n.SettingsAccountSyncTechnologyUnreachable(),
        _ => L10n.SettingsAccountSyncTechnologyChecking(),
    };

    public static string Detail(MailSyncTechnology technology) => technology switch
    {
        MailSyncTechnology.Optimised => L10n.SettingsAccountSyncTechnologyOptimisedDetail(),
        MailSyncTechnology.Fallback => L10n.SettingsAccountSyncTechnologyFallbackDetail(),
        MailSyncTechnology.Unreachable => L10n.SettingsAccountSyncTechnologyUnreachableDetail(),
        _ => L10n.SettingsAccountSyncTechnologyCheckingDetail(),
    };
}
