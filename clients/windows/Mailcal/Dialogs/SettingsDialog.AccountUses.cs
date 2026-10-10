// An account's page, its first questions: what the account is used for (docs/accounts.md rule 13),
// which other accounts fill what it is not (rule 12), and, for one that signs in at its provider,
// signing in again (docs/provider-oauth.md rule 11).
//
// A nested ContentDialog is not allowed, so switching a use off is confirmed inline, under its
// switch, as the database reset is.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>A switch per use the account's kind offers.</summary>
    private UIElement UsesGroup(AccountEntry entry)
    {
        var stack = new StackPanel { Spacing = 8 };
        foreach (var use in entry.Uses)
        {
            stack.Children.Add(UseRow(entry, use.Capability));
        }
        return Group(L10n.SettingsAccountUsesHeading(), L10n.SettingsAccountUsesDescription(), stack);
    }

    private StackPanel UseRow(AccountEntry entry, AccountCapability capability)
    {
        var rule = AccountSettingsRules.Switch(entry, capability);
        var row = new StackPanel { Spacing = 2 };
        var toggle = new ToggleSwitch { Header = UseName(capability), IsOn = rule.Active, IsEnabled = rule.Enabled };
        AutomationProperties.SetAutomationId(toggle, $"AccountUse_{capability}");
        row.Children.Add(toggle);
        if (UseNoteText(rule.Note) is { } note)
        {
            row.Children.Add(Description(note));
        }

        // Switching off deletes what the device holds of the use, so it is asked first; cancelling
        // puts the switch back.
        var confirm = new StackPanel { Spacing = 8, Visibility = Visibility.Collapsed };
        confirm.Children.Add(Heading(L10n.AccountUseOffTitle(UseName(capability))));
        confirm.Children.Add(new TextBlock { Text = L10n.AccountUseOffMessage(), TextWrapping = TextWrapping.Wrap });
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var cancel = new Button { Content = L10n.ActionCancel() };
        var off = new Button { Content = L10n.ActionSwitchOff() };
        buttons.Children.Add(cancel);
        buttons.Children.Add(off);
        confirm.Children.Add(buttons);
        row.Children.Add(confirm);

        var restoring = false;
        toggle.Toggled += (_, _) =>
        {
            if (_rebuilding || restoring)
            {
                return;
            }
            if (toggle.IsOn && rule.Asks)
            {
                // Chosen, and waiting on the provider: switching it on asks again.
                _ = SignInAgainFromPageAsync(entry, [capability]);
            }
            else if (toggle.IsOn)
            {
                _ = SetUseAsync(entry, capability, true);
            }
            else
            {
                toggle.IsEnabled = false;
                confirm.Visibility = Visibility.Visible;
            }
        };
        cancel.Click += (_, _) =>
        {
            confirm.Visibility = Visibility.Collapsed;
            restoring = true;
            toggle.IsOn = true;
            restoring = false;
            toggle.IsEnabled = true;
        };
        off.Click += (_, _) =>
        {
            confirm.Visibility = Visibility.Collapsed;
            _ = SetUseAsync(entry, capability, false);
        };
        return row;
    }

    private static string? UseNoteText(UseNote note) => note switch
    {
        UseNote.Last => L10n.SettingsAccountUseLast(),
        UseNote.NeedsContacts => L10n.SettingsAccountColleaguesNeedsContacts(),
        UseNote.Withheld => L10n.SettingsAccountUseWithheld(),
        _ => null,
    };

    private async Task SetUseAsync(AccountEntry entry, AccountCapability capability, bool on)
    {
        var (outcome, error) = await _model.SetAccountUseAsync(entry.Id, capability, on);
        if (error is not null)
        {
            NotifyAccounts(ChangeFailed(error));
            return;
        }
        switch (outcome)
        {
            // The provider has not granted it: the person switching it on is the person who
            // would be asked, so they are asked now.
            case UseChangeOutcome.AskProvider:
                await SignInAgainFromPageAsync(entry, [capability]);
                break;
            case UseChangeOutcome.NeedsEndpoint:
                NotifyAccounts(ChangeFailed(L10n.SettingsAccountUseNeedsEndpoint()));
                break;
            default:
                NotifyAccounts(null);
                break;
        }
    }

    /// <summary>A picker per link slot the account can hold.</summary>
    private UIElement LinksGroup(AccountEntry entry, IReadOnlyList<LinkPicker> pickers)
    {
        var stack = new StackPanel { Spacing = 12 };
        foreach (var picker in pickers)
        {
            stack.Children.Add(LinkRow(entry, picker));
        }
        return Group(L10n.SettingsAccountLinksHeading(), L10n.SettingsAccountLinksDescription(), stack);
    }

    private StackPanel LinkRow(AccountEntry entry, LinkPicker picker)
    {
        var row = new StackPanel { Spacing = 4 };
        var box = new ComboBox { Header = LinkTitle(picker.Slot), MinWidth = 260 };
        AutomationProperties.SetName(box, LinkTitle(picker.Slot));
        AutomationProperties.SetAutomationId(box, $"AccountLink_{picker.Slot}");
        box.Items.Add(L10n.SettingsAccountLinkNone());
        foreach (var option in picker.Options)
        {
            box.Items.Add(option.Address);
        }
        box.SelectedIndex = picker.Selected is { } selected ? selected + 1 : 0;
        box.SelectionChanged += (_, _) =>
        {
            if (_rebuilding || box.SelectedIndex < 0)
            {
                return;
            }
            var target = box.SelectedIndex == 0 ? null : picker.Options[box.SelectedIndex - 1].Id;
            _ = SetLinkAsync(entry.Id, picker.Slot, target);
        };
        row.Children.Add(box);
        // A suggestion is named under the picker, with a button that picks it: the person confirms
        // it, it is never linked for them (docs/accounts.md rule 12).
        if (picker.Suggested is { } suggested)
        {
            var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            var text = Description(L10n.SettingsAccountLinkSuggested(picker.Options[suggested].Address));
            text.VerticalAlignment = VerticalAlignment.Center;
            line.Children.Add(text);
            var link = new Button { Content = L10n.ActionLink() };
            AutomationProperties.SetAutomationId(link, $"AccountLinkSuggested_{picker.Slot}");
            link.Click += (_, _) => box.SelectedIndex = suggested + 1;
            line.Children.Add(link);
            row.Children.Add(line);
        }
        return row;
    }

    internal static string LinkTitle(LinkSlot slot) => slot switch
    {
        LinkSlot.Calendar => L10n.SettingsAccountLinkCalendar(),
        LinkSlot.Contacts => L10n.SettingsAccountLinkContacts(),
        _ => L10n.SettingsAccountLinkMail(),
    };

    private async Task SetLinkAsync(string accountId, LinkSlot slot, string? target)
    {
        var error = await _model.SetAccountLinkAsync(accountId, slot, target);
        NotifyAccounts(error is null ? null : ChangeFailed(error));
    }

    /// <summary>"Sign in again", for an account that signs in at its provider's page: the remedy
    /// for an expired sign-in and for a permission the provider withheld.</summary>
    private UIElement SignInGroup(AccountEntry entry, bool expired)
    {
        var description = expired
            ? L10n.SigninExpiredPrompt(entry.Address)
            : AccountSettingsRules.NeedsPermission(entry)
                ? L10n.SettingsAccountNeedsPermission()
                : L10n.SettingsAccountSigninDescription();
        var button = new Button { Content = L10n.SigninExpiredAction() };
        AutomationProperties.SetAutomationId(button, "AccountSignInAgain");
        button.Click += (_, _) => _ = SignInAgainFromPageAsync(entry, []);
        return Group(L10n.SettingsAccountSigninHeading(), description, button);
    }

    /// <summary>Signs the account in again at its provider, asking for what it is used for plus
    /// <paramref name="adding"/>, and says how it ended. A cancelled sign-in says nothing.</summary>
    private async Task SignInAgainFromPageAsync(AccountEntry entry, AccountCapability[] adding)
    {
        SayOnAccounts(new AccountNotice(
            InfoBarSeverity.Informational, string.Empty, L10n.SettingsAccountSigninWaiting(entry.Address)));
        var ended = await _model.SignInAgainAsync(entry.Id, adding);
        NotifyAccounts(ended switch
        {
            { SignedIn: true } => SignedInNotice(entry.Id),
            { Error: { } error } => new AccountNotice(InfoBarSeverity.Error, L10n.SettingsAccountSigninFailed(), error),
            _ => null,
        });
    }

    /// <summary>What a completed sign-in says: that it worked, or, when the provider still did
    /// not allow a use the account is chosen for, which ones, since the sign-in alone does not
    /// mean the person got what they asked for.</summary>
    private AccountNotice SignedInNotice(string accountId)
    {
        var entry = _model.AccountsSnapshot().Accounts.FirstOrDefault(account => account.Id == accountId);
        if (entry is not null && AccountSettingsRules.Withheld(entry) is { Count: > 0 } withheld)
        {
            return new AccountNotice(
                InfoBarSeverity.Warning,
                L10n.SetupWithheldTitle(),
                L10n.SetupWithheldDetail(string.Join(", ", withheld.Select(UseName))));
        }
        return new AccountNotice(InfoBarSeverity.Success, L10n.SettingsAccountSignedIn(), string.Empty);
    }
}
