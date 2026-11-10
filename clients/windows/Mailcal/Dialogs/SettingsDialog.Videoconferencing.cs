// Settings → Videoconferencing (docs/settings.md slot 4): the accounts that make video meetings,
// while there are any, then the person's Jitsi server (docs/conferencing.md, rule 13). The server
// is empty until they fill it, and checked by the core before it is stored, so an address it
// refuses is said here and nothing changes. Once one is stored, every calendar whose provider
// writes a meeting offers Jitsi in the editor. Last, while any account offers a meeting, the one
// each account's new events take once they have an invitee (rule 10).
//
// An account's row opens its page rather than carrying a switch of its own: two switches for one
// setting would be two places to change it, and switching Teams on can need the Microsoft sign-in,
// which the page holds.

using Allodia.Mailcal.Calendar;
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

    // The default per account, refilled when an account or the Jitsi server changes what is on
    // offer.
    private StackPanel? _meetingDefaults;
    private string? _meetingDefaultsDrawn;

    private UIElement BuildVideoconferencing()
    {
        _meetingAccounts = new StackPanel();
        _meetingAccountsDrawn = null;
        _meetingDefaults = new StackPanel();
        _meetingDefaultsDrawn = null;
        RefreshMeetingAccounts();
        return new StackPanel
        {
            Spacing = 20,
            Children = { _meetingAccounts, JitsiServerGroup(), _meetingDefaults },
        };
    }

    /// <summary>Draws the accounts that make meetings again, when what they show has changed.</summary>
    private void RefreshMeetingAccounts()
    {
        RefreshMeetingDefaults();
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

    /// <summary>Draws the default per account again, when what the core lists has changed.</summary>
    private void RefreshMeetingDefaults()
    {
        if (_meetingDefaults is not { } host)
        {
            return;
        }
        var rows = _model.MeetingDefaults();
        var drawn = string.Join('\n', rows.Select(row =>
            $"{row.Account}\t{row.AccountAddress}\t{row.Chosen}\t"
            + string.Join('\t', row.Choices.Select(choice => $"{choice.Id}|{choice.Name}|{choice.Through}"))));
        if (drawn == _meetingDefaultsDrawn)
        {
            return;
        }
        _meetingDefaultsDrawn = drawn;
        host.Children.Clear();
        if (rows.Length == 0)
        {
            return;
        }
        var list = new StackPanel { Spacing = 8 };
        foreach (var row in rows)
        {
            list.Children.Add(DefaultMeetingRow(row));
        }
        host.Children.Add(Group(L10n.SettingsDefaultMeeting(), L10n.SettingsDefaultMeetingHint(), list));
    }

    /// <summary>One account: its address and a picker, None first, then what its calendars
    /// offer.</summary>
    private Grid DefaultMeetingRow(MeetingDefault row)
    {
        var options = new List<DefaultMeetingOption> { new(null, L10n.EventMeetingNone()) };
        options.AddRange(row.Choices.Select(choice => new DefaultMeetingOption(
            choice.Id,
            new MeetingRowOption(new MeetingSelection.Choice(choice.Id), choice.Name, choice.Through)
                .Label(L10n.EventMeetingNone(), L10n.EventVideoMeeting(), L10n.EventMeetingThrough))));
        var box = new ComboBox
        {
            MinWidth = 240,
            VerticalAlignment = VerticalAlignment.Center,
            ItemsSource = options,
            SelectedItem = options.FirstOrDefault(option => option.Id == row.Chosen) ?? options[0],
        };
        AutomationProperties.SetAutomationId(box, $"DefaultMeeting_{row.Account}");
        AutomationProperties.SetName(box, row.AccountAddress);
        box.SelectionChanged += (_, _) =>
        {
            if (box.SelectedItem is DefaultMeetingOption option)
            {
                _model.SetDefaultMeeting(row.Account, option.Id);
            }
        };
        var address = new TextBlock
        {
            Text = row.AccountAddress,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        Grid.SetColumn(box, 1);
        grid.Children.Add(address);
        grid.Children.Add(box);
        return grid;
    }

    // One entry in an account's picker; ToString is the label so the ComboBox shows it directly.
    // A null Id is "None".
    private sealed record DefaultMeetingOption(string? Id, string Label)
    {
        public override string ToString() => Label;
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
                // A stored or forgotten server changes what every account offers.
                RefreshMeetingDefaults();
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
