// An account's page, the accounts taking a use from it (docs/accounts.md rule 12): each named, with
// what it takes and Stop linking, which asks first as a removal does. The rows are the core's
// (`account_page`), worded by AccountLinkedRules.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>The words of the Linked to group and of Stop linking's question.</summary>
    private static LinkedFromWords LinkedFromText() => new(
        AccountNameText.Words,
        new ByUses(
            L10n.SettingsAccountLinkedFromCalendar(),
            L10n.SettingsAccountLinkedFromContacts(),
            L10n.SettingsAccountLinkedFromBoth()),
        new ByUses(
            L10n.SettingsAccountStopLinkingCalendar(),
            L10n.SettingsAccountStopLinkingContacts(),
            L10n.SettingsAccountStopLinkingBoth()),
        L10n.SettingsAccountStopLinkingLeftWithout(),
        L10n.SettingsAccountStopLinkingTitle);

    /// <summary>One row per account taking a use from this one.</summary>
    private UIElement LinkedFromGroup(AccountEntry entry, IReadOnlyList<AccountLinkedFrom> linked)
    {
        var words = LinkedFromText();
        var panel = new StackPanel { Spacing = 6 };
        panel.Children.Add(Heading(L10n.SettingsAccountLinkedFromHeading()));
        var stack = new StackPanel { Spacing = 12 };
        for (var at = 0; at < linked.Count; at++)
        {
            stack.Children.Add(LinkedFromRow(entry, linked[at], words, $"AccountStopLinking_{at}"));
        }
        panel.Children.Add(stack);
        return panel;
    }

    private StackPanel LinkedFromRow(AccountEntry entry, AccountLinkedFrom taking, LinkedFromWords words, string automationId)
    {
        var row = AccountLinkedRules.Row(entry.Id, taking, words);
        var text = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(new TextBlock { Text = row.Title, TextWrapping = TextWrapping.Wrap });
        text.Children.Add(Description(row.Line));
        var line = new Grid { ColumnSpacing = 12 };
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        line.Children.Add(text);
        StackPanel? stop = null;
        var confirm = new ConfirmPanel(
            row.ConfirmTitle, row.ConfirmMessage, L10n.SettingsAccountStopLinking(), automationId, yes =>
            {
                _confirmOpen = null;
                stop!.Visibility = Visibility.Visible;
                if (AccountLinkedRules.StopAnswer(yes, entry.Id, taking) is { } intent)
                {
                    _model.ChangeAccount(intent);
                }
            });
        stop = ControlComponents.Button(L10n.SettingsAccountStopLinking(), row.Stop, automationId, () =>
        {
            _confirmOpen = row.Asking;
            stop!.Visibility = Visibility.Collapsed;
            confirm.Show();
        });
        stop.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(stop, 1);
        line.Children.Add(stop);
        // Still asked when the page is drawn again; Stop linking waits while another change runs.
        if (_confirmOpen == row.Asking)
        {
            stop.Visibility = Visibility.Collapsed;
            confirm.Show();
        }
        confirm.Enabled = row.ConfirmEnabled;
        var panel = new StackPanel { Spacing = 8 };
        panel.Children.Add(line);
        panel.Children.Add(confirm.Element);
        return panel;
    }
}
