// The per-calendar visibility + colour manager, the Windows twin of Android's CalendarManagerScreen
// and macOS's calendar manager. A modal list of every calendar, grouped by account: a colour swatch
// that opens the palette, the name, and a visibility checkbox.
//
// State lives in the CORE. Both writes (`set_calendar_visible` / `set_calendar_color`) are applied at
// page-read time, no sync, no network, and the core re-signals Surface::Calendar, so an unticked
// calendar disappears from the grid immediately (docs/calendar.md §5). The colour is snapped to the
// nearest palette entry in the core, so a client cannot introduce an off-palette colour (Allodia
// Orange is deliberately absent, it means "action").
//
// Built imperatively in code-behind, like SettingsDialog, so the tree is deterministic and there is
// no client-side state to drift from the core: every change writes back and re-reads.
using System;
using System.Globalization;
using System.Linq;
using Allodia.Mailcal.Services;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.UI;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>A modal per-calendar visibility + colour editor.</summary>
public sealed class CalendarManagerDialog : ContentDialog
{
    private readonly MailboxModel _model;
    private readonly string[] _palette;
    private readonly StackPanel _list = new() { Spacing = 14 };

    // Guards the programmatic IsChecked assignments during a (re)build from firing the change
    // handlers, the same pattern SettingsDialog uses.
    private bool _rebuilding;

    /// <summary>Builds the manager over the shared model.</summary>
    public CalendarManagerDialog(MailboxModel model)
    {
        _model = model;
        _palette = MailboxModel.CalendarPalette();
        Title = L10n.CalendarManage();
        CloseButtonText = L10n.ActionDone();
        DefaultButton = ContentDialogButton.Close;
        Content = new ScrollViewer { Content = _list, MinWidth = 380, MaxHeight = 460 };
        Build();
    }

    private bool Dark => ActualTheme == ElementTheme.Dark;

    // (Re)builds the list from the core. Called on open and after a colour change (a reset needs the
    // server's resolved colour back, which only a re-pull knows).
    private void Build()
    {
        _rebuilding = true;
        _list.Children.Clear();

        var calendars = _model.Calendars();
        if (calendars.Length == 0)
        {
            _list.Children.Add(new TextBlock
            {
                Text = L10n.CalendarManageEmpty(),
                Opacity = 0.7,
                TextWrapping = TextWrapping.Wrap,
            });
            _rebuilding = false;
            return;
        }

        // Grouped by account: a calendar id is unique only within its account, so two accounts each
        // with a "work" calendar must read as two rows under two headings.
        foreach (var group in calendars.GroupBy(c => c.Account))
        {
            var account = _model.Accounts.FirstOrDefault(a => a.Id == group.Key);
            _list.Children.Add(new TextBlock
            {
                Text = account?.Email ?? group.Key,
                FontWeight = FontWeights.SemiBold,
            });
            foreach (var row in group)
            {
                _list.Children.Add(BuildRow(row));
            }
        }

        _rebuilding = false;
    }

    private UIElement BuildRow(CalendarRow row)
    {
        var grid = new Grid { ColumnSpacing = 10, Padding = new Thickness(0, 2, 0, 2) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        // The colour swatch, a button that opens the palette. Drawn in the theme's own resolved
        // colour, the same one the grid paints with, so the manager and the grid never disagree.
        var swatch = new Border
        {
            Width = 18,
            Height = 18,
            CornerRadius = new CornerRadius(4),
            Background = new SolidColorBrush(ParseHex(Dark ? row.Color.Dark.Background : row.Color.Light.Background)),
        };
        var swatchButton = new Button
        {
            Padding = new Thickness(4),
            Content = swatch,
            Flyout = ColorFlyout(row),
        };
        AutomationProperties.SetName(swatchButton, L10n.CalendarPickColor(row.Name));
        Grid.SetColumn(swatchButton, 0);
        grid.Children.Add(swatchButton);

        var name = new TextBlock
        {
            Text = row.Name,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };
        Grid.SetColumn(name, 1);
        grid.Children.Add(name);

        // The visibility toggle. Set directly (no rebuild): the checkbox already shows the new state,
        // and the grid re-pulls on the core's Surface::Calendar signal.
        var visible = new CheckBox { IsChecked = row.Visible, MinWidth = 0 };
        AutomationProperties.SetName(visible, row.Name);
        visible.Checked += (_, _) =>
        {
            if (!_rebuilding)
            {
                _model.SetCalendarVisible(row.Account, row.Id, true);
            }
        };
        visible.Unchecked += (_, _) =>
        {
            if (!_rebuilding)
            {
                _model.SetCalendarVisible(row.Account, row.Id, false);
            }
        };
        Grid.SetColumn(visible, 2);
        grid.Children.Add(visible);

        return grid;
    }

    private Flyout ColorFlyout(CalendarRow row)
    {
        var flyout = new Flyout();
        var panel = new StackPanel { Spacing = 8 };

        // The palette, in rows of five. The current colour wears a ring.
        var swatches = new StackPanel { Spacing = 6 };
        StackPanel? line = null;
        for (var i = 0; i < _palette.Length; i++)
        {
            if (i % 5 == 0)
            {
                line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
                swatches.Children.Add(line);
            }

            var hex = _palette[i];
            var selected = string.Equals(row.Color.Hex, hex, StringComparison.OrdinalIgnoreCase);
            var button = new Button
            {
                Width = 30,
                Height = 30,
                Padding = new Thickness(0),
                Background = new SolidColorBrush(ParseHex(hex)),
                BorderThickness = new Thickness(selected ? 3 : 1),
            };
            button.Click += (_, _) =>
            {
                flyout.Hide();
                PickColor(row, hex);
            };
            line!.Children.Add(button);
        }
        panel.Children.Add(swatches);

        // Back to the server's own colour.
        var reset = new Button
        {
            Content = L10n.CalendarColorReset(),
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        reset.Click += (_, _) =>
        {
            flyout.Hide();
            PickColor(row, null);
        };
        panel.Children.Add(reset);

        flyout.Content = panel;
        return flyout;
    }

    private void PickColor(CalendarRow row, string? hex)
    {
        _model.SetCalendarColor(row.Account, row.Id, hex);
        // Rebuild after the flyout has closed, so the swatch shows the newly resolved colour and the
        // selection ring moves. A reset in particular needs the server's colour, which only a re-pull
        // knows.
        _ = DispatcherQueue.TryEnqueue(Build);
    }

    // "#rrggbb" -> an opaque colour. Neutral grey on anything unexpected, rather than throwing inside
    // a draw. (The core only ever emits well-formed palette hexes, so the fallback is belt-and-braces.)
    private static Color ParseHex(string hex)
    {
        if (hex.Length == 7 && hex[0] == '#'
            && byte.TryParse(hex.AsSpan(1, 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var r)
            && byte.TryParse(hex.AsSpan(3, 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var g)
            && byte.TryParse(hex.AsSpan(5, 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var b))
        {
            return Color.FromArgb(255, r, g, b);
        }
        return Color.FromArgb(255, 128, 128, 128);
    }
}
