// The setup form's last step: linking the account just added to another, before the form closes
// (docs/onboarding.md, "After the connect"). The pickers are the account page's own, from the same
// snapshot; a suggested account starts picked and is linked only when the person continues. What
// the step holds is SetupLinkStep and SetupFlow, which the unit suite drives.

using Allodia.Mailcal;
using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    /// <summary>Draws the link step when the flow is on it, and leaves it for the address when
    /// "Add another account" took the flow back there.</summary>
    private void OnSetupLinksChanged()
    {
        if (Model?.SetupLinks is { } step)
        {
            DrawLinks(step);
            DetectPanel.Visibility = Visibility.Collapsed;
            SetupPanel.Visibility = Visibility.Collapsed;
            LinksPanel.Visibility = Visibility.Visible;
            return;
        }
        if (LinksPanel.Visibility == Visibility.Visible)
        {
            LinksPanel.Visibility = Visibility.Collapsed;
            ResetToDetect();
        }
    }

    private void DrawLinks(SetupLinkStep step)
    {
        LinksPanel.Children.Clear();
        var title = new TextBlock
        {
            Text = L10n.SetupLinksTitle(),
            Style = (Style)Application.Current.Resources["TitleTextBlockStyle"],
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetHeadingLevel(title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level1);
        LinksPanel.Children.Add(title);
        LinksPanel.Children.Add(Secondary(L10n.SettingsAccountLinksDescription(), "BodyTextBlockStyle"));
        for (var index = 0; index < step.Pickers.Count; index++)
        {
            var picker = step.Pickers[index];
            var at = index;
            var box = new ComboBox { Header = SettingsDialog.LinkTitle(picker.Slot), MinWidth = 260 };
            AutomationProperties.SetAutomationId(box, $"SetupLink_{picker.Slot}");
            box.Items.Add(L10n.SettingsAccountLinkNone());
            foreach (var option in picker.Options)
            {
                box.Items.Add(option.Address);
            }
            box.SelectedIndex = step.Picked[index] is { } picked ? picked + 1 : 0;
            box.SelectionChanged += (_, _) =>
            {
                if (box.SelectedIndex >= 0)
                {
                    step.Pick(at, box.SelectedIndex == 0 ? null : box.SelectedIndex - 1);
                }
            };
            LinksPanel.Children.Add(box);
            if (picker.Suggested is { } suggested)
            {
                LinksPanel.Children.Add(Secondary(
                    L10n.SettingsAccountLinkSuggested(picker.Options[suggested].Address), "CaptionTextBlockStyle"));
            }
        }
        LinksPanel.Children.Add(Secondary(L10n.SetupLinksNote(), "CaptionTextBlockStyle"));
        var another = new Button { Content = L10n.SetupLinksAdd() };
        AutomationProperties.SetAutomationId(another, "SetupLinksAdd");
        another.Click += (_, _) => Model?.SetupAddLinkedAccount();
        LinksPanel.Children.Add(another);

        var actions = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Right,
            Spacing = 8,
        };
        var skip = new Button { Content = L10n.SetupSenderNameSkip() };
        AutomationProperties.SetAutomationId(skip, "SetupLinksSkip");
        var link = new Button
        {
            Content = L10n.SetupDetectAction(),
            Style = (Style)Application.Current.Resources["AccentButtonStyle"],
        };
        AutomationProperties.SetAutomationId(link, "SetupLinksDone");
        void Done(bool linking)
        {
            skip.IsEnabled = link.IsEnabled = another.IsEnabled = false;
            _ = Model?.SetupLinksDoneAsync(linking);
        }
        skip.Click += (_, _) => Done(false);
        link.Click += (_, _) => Done(true);
        actions.Children.Add(skip);
        actions.Children.Add(link);
        LinksPanel.Children.Add(actions);
    }

    private static TextBlock Secondary(string text, string style) => new()
    {
        Text = text,
        Style = (Style)Application.Current.Resources[style],
        TextWrapping = TextWrapping.Wrap,
        Foreground = (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"],
    };
}
