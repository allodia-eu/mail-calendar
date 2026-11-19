// The calendar pane's list of calendars (docs/calendar-pane.md, rules 6 to 9): a group under each
// account's address, opened and shut through the core; a row per calendar that toggles it, with a
// menu that recolours it. Drawn from the pull; nothing here is kept between pulls.

using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class CalendarPane
{    private void DrawGroups(CalendarGroup[] groups)
    {
        Groups.Children.Clear();
        if (groups.Length == 0)
        {
            Groups.Children.Add(new TextBlock
            {
                Text = L10n.CalendarManageEmpty(),
                Margin = new Thickness(16, 8, 16, 8),
                TextWrapping = TextWrapping.Wrap,
                Style = (Style)Resources["PaneCaptionStyle"],
                HorizontalAlignment = HorizontalAlignment.Left,
            });
            return;
        }
        foreach (var group in groups)
        {
            Groups.Children.Add(Heading(group));
            if (!group.Expanded)
            {
                continue;
            }
            foreach (var row in group.Calendars)
            {
                Groups.Children.Add(CalendarRowView(row));
            }
        }
    }

    // An account's heading, drawn as the folder pane draws an account row: heavier, behind a
    // disclosure chevron, opening and shutting its calendars (rule 7).
    private Button Heading(CalendarGroup group)
    {
        var line = new Grid { ColumnSpacing = 8 };
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var chevron = new FontIcon
        {
            Glyph = group.Expanded ? "" : "", // ChevronDown / ChevronRight
            FontSize = 12,
            Style = (Style)Resources["PaneSecondaryIconStyle"],
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetAccessibilityView(chevron, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        line.Children.Add(chevron);
        // The address, and beneath it the mark the account's folder-tree row carries while a notice
        // is about it (docs/notices.md rule 6).
        var mark = _model?.MarkOf(group.Account);
        var address = new StackPanel { Padding = new Thickness(0, 4, 0, 4), VerticalAlignment = VerticalAlignment.Center };
        address.Children.Add(new TextBlock
        {
            Text = group.Address,
            FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
            TextTrimming = TextTrimming.CharacterEllipsis,
        });
        address.Children.Add(new AccountMarkLine { Mark = mark });
        Grid.SetColumn(address, 1);
        line.Children.Add(address);

        var heading = new Button
        {
            Style = (Style)Resources["PaneFlatButtonStyle"],
            MinHeight = 36,
            Margin = new Thickness(4, 2, 4, 2),
            Padding = new Thickness(12, 0, 12, 0),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Content = line,
        };
        ToolTipService.SetToolTip(heading, group.Address);
        AutomationProperties.SetName(heading, group.Address);
        AutomationProperties.SetAutomationId(heading, $"CalendarPaneGroup_{group.Account}");
        AutomationProperties.SetItemStatus(heading, mark?.Detail ?? string.Empty);
        heading.Click += (_, _) => _model?.SetCalendarGroupExpanded(group.Account, !group.Expanded);
        return heading;
    }

    // A calendar: the round colour check, the name and a lock when it cannot be written. The whole
    // row toggles (rule 8); its menu recolours it (rule 9).
    private CheckBox CalendarRowView(CalendarRow row)
    {
        var line = new Grid { ColumnSpacing = 10 };
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        line.Children.Add(ColourCheck(row, ActualTheme == ElementTheme.Dark ? row.Color.Dark : row.Color.Light));

        var name = new TextBlock
        {
            Text = row.Name,
            TextTrimming = TextTrimming.CharacterEllipsis,
            VerticalAlignment = VerticalAlignment.Center,
        };
        Grid.SetColumn(name, 1);
        line.Children.Add(name);

        if (!row.CanWrite)
        {
            var locked = new FontIcon
            {
                Glyph = "", // Lock
                FontSize = 12,
                Style = (Style)Resources["PaneSecondaryIconStyle"],
                VerticalAlignment = VerticalAlignment.Center,
            };
            ToolTipService.SetToolTip(locked, L10n.CalendarPaneReadOnly());
            AutomationProperties.SetAccessibilityView(locked, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
            Grid.SetColumn(locked, 2);
            line.Children.Add(locked);
        }

        var check = new CheckBox
        {
            Style = (Style)Resources["PaneRowStyle"],
            IsChecked = row.Visible,
            Content = line,
            ContextFlyout = RowMenu(row),
        };
        AutomationProperties.SetName(check, L10n.A11yCalendarPaneCalendar(row.Name, row.AccountAddress));
        AutomationProperties.SetAutomationId(check, $"CalendarPaneCalendar_{row.Account}_{row.Id}");
        if (!row.CanWrite)
        {
            AutomationProperties.SetHelpText(check, L10n.CalendarPaneReadOnly());
        }
        // On the change rather than the click: UI Automation's toggle, which is a screen reader's,
        // changes a check box without clicking it. What the row then shows is the core's answer,
        // which the next pull redraws.
        check.Checked += (_, _) => _model?.SetCalendarVisible(row.Account, row.Id, true);
        check.Unchecked += (_, _) => _model?.SetCalendarVisible(row.Account, row.Id, false);
        return check;
    }

    // Filled with the calendar's colour and a tick while shown, a ring in that colour while hidden:
    // the theme's swatch, whose label colour the core made readable on it.
    private static Grid ColourCheck(CalendarRow row, Swatch swatch)
    {
        var brush = new SolidColorBrush(CalendarColors.Parse(swatch.Background));
        var check = new Grid { Width = 16, Height = 16, VerticalAlignment = VerticalAlignment.Center };
        check.Children.Add(new Ellipse
        {
            Width = 16,
            Height = 16,
            Fill = row.Visible ? brush : null,
            Stroke = brush,
            StrokeThickness = 2,
        });
        if (row.Visible)
        {
            check.Children.Add(new FontIcon
            {
                Glyph = "", // CheckMark
                FontSize = 10,
                Foreground = new SolidColorBrush(CalendarColors.Parse(swatch.Text)),
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            });
        }
        AutomationProperties.SetAccessibilityView(check, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        return check;
    }

    private MenuFlyout RowMenu(CalendarRow row)
    {
        var menu = new MenuFlyout();
        var colour = new MenuFlyoutItem { Text = L10n.CalendarPaneColor() };
        colour.Click += (_, _) => ShowPalette(row);
        var reset = new MenuFlyoutItem { Text = L10n.CalendarColorReset() };
        reset.Click += (_, _) => _model?.SetCalendarColor(row.Account, row.Id, null);
        menu.Items.Add(colour);
        menu.Items.Add(reset);
        return menu;
    }

    // The palette, in rows of five, the current colour ringed; opened on the row it recolours.
    private void ShowPalette(CalendarRow row)
    {
        var anchor = Groups.Children
            .OfType<FrameworkElement>()
            .FirstOrDefault(e => AutomationProperties.GetAutomationId(e) == $"CalendarPaneCalendar_{row.Account}_{row.Id}");
        var flyout = new Flyout();
        var swatches = new StackPanel { Spacing = 6 };
        StackPanel? line = null;
        var palette = MailboxModel.CalendarPalette();
        for (var i = 0; i < palette.Length; i++)
        {
            if (i % 5 == 0)
            {
                line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
                swatches.Children.Add(line);
            }
            var hex = palette[i];
            var selected = string.Equals(row.Color.Hex, hex, StringComparison.OrdinalIgnoreCase);
            var swatch = new Button
            {
                Width = 30,
                Height = 30,
                Padding = new Thickness(0),
                CornerRadius = new CornerRadius(15),
                Background = new SolidColorBrush(CalendarColors.Parse(hex)),
                BorderThickness = new Thickness(selected ? 3 : 1),
            };
            AutomationProperties.SetName(swatch, hex);
            swatch.Click += (_, _) =>
            {
                flyout.Hide();
                _model?.SetCalendarColor(row.Account, row.Id, hex);
            };
            line!.Children.Add(swatch);
        }
        flyout.Content = swatches;
        flyout.ShowAt(anchor ?? Groups);
    }
}
