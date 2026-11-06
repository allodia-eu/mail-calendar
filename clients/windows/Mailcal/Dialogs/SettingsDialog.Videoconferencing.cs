// Settings → Videoconferencing (docs/settings.md slot 4): the accounts that make video meetings,
// while there are any, then the person's Jitsi server (docs/conferencing.md, rule 13). The server
// is empty until they fill it, and checked by the core before it is stored, so an address it
// refuses is said here and nothing changes. Once one is stored, every calendar whose provider
// writes a meeting offers Jitsi in the editor.
//
// An account's row opens its page rather than carrying a switch of its own: two switches for one
// setting would be two places to change it, and switching Teams on can need the Microsoft sign-in,
// which the page holds.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;
using Windows.System;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The group of accounts that make meetings, refilled when the snapshot changes them, so the
    // Jitsi field beside it keeps what is being typed.
    private StackPanel? _meetingAccounts;
    private string? _meetingAccountsDrawn;

    private UIElement BuildVideoconferencing()
    {
        _meetingAccounts = new StackPanel();
        _meetingAccountsDrawn = null;
        RefreshMeetingAccounts();
        return new StackPanel { Spacing = 20, Children = { _meetingAccounts, JitsiServerGroup() } };
    }

    /// <summary>Draws the accounts that make meetings again, when what they show has changed.</summary>
    private void RefreshMeetingAccounts()
    {
        if (_meetingAccounts is not { } host)
        {
            return;
        }
        var rows = _model.AccountsSnapshot().MeetingAccounts;
        var drawn = MeetingAccountLabels.Fingerprint(rows, AccountNameText.Of);
        if (drawn == _meetingAccountsDrawn)
        {
            return;
        }
        _meetingAccountsDrawn = drawn;
        host.Children.Clear();
        if (rows.Length == 0)
        {
            return;
        }
        var list = new StackPanel { Spacing = 4 };
        foreach (var row in rows)
        {
            list.Children.Add(MeetingAccountRow(row));
        }
        host.Children.Add(Group(L10n.SettingsMeetingAccounts(), L10n.SettingsMeetingAccountsHint(), list));
    }

    /// <summary>One account: the service, the account beneath it, its state and an arrow to its
    /// page.</summary>
    private Button MeetingAccountRow(MeetingAccount row)
    {
        var name = AccountNameText.Of(row.Account);
        var state = MeetingAccountLabels.State(row.State, new MeetingStateWords(
            L10n.SettingsToggleOn(), L10n.SettingsToggleOff(),
            L10n.SettingsMeetingAccountNeedsPermission(), L10n.SettingsMeetingAccountNotOffered()));
        var text = new StackPanel { Spacing = 2 };
        text.Children.Add(new TextBlock { Text = row.Service });
        text.Children.Add(new TextBlock
        {
            Text = name,
            Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
            Foreground = _brushes.Of(ThemePalette.SecondaryText),
            TextWrapping = TextWrapping.Wrap,
        });
        var stateText = new TextBlock
        {
            Text = state,
            Foreground = _brushes.Of(ThemePalette.SecondaryText),
            VerticalAlignment = VerticalAlignment.Center,
        };
        var arrow = new FontIcon { Glyph = "", FontSize = 12, VerticalAlignment = VerticalAlignment.Center };
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        Grid.SetColumn(stateText, 1);
        Grid.SetColumn(arrow, 2);
        grid.Children.Add(text);
        grid.Children.Add(stateText);
        grid.Children.Add(arrow);
        var button = new Button
        {
            Content = grid,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Padding = new Thickness(12, 8, 12, 8),
        };
        AutomationProperties.SetAutomationId(button, $"MeetingAccount_{row.Account.Id}");
        AutomationProperties.SetName(button, MeetingAccountLabels.Spoken(row.Service, name, state));
        button.Click += (_, _) => OpenAccountFromOtherCategory(row.Account.Id);
        return button;
    }

    /// <summary>Opens <paramref name="accountId"/>'s page in Accounts, with the category list
    /// following.</summary>
    private void OpenAccountFromOtherCategory(string accountId)
    {
        _rebuilding = true;
        _categories.SelectedIndex = Array.FindIndex(Categories(), entry => entry.Tag == "accounts");
        _rebuilding = false;
        OpenAccountPage(accountId);
    }

    private UIElement JitsiServerGroup()
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
