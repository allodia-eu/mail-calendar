// An account's page, its first questions: what the account is used for (docs/accounts.md rule 13),
// which other accounts fill what it is not (rule 12), and, for one that signs in at its provider,
// signing in again (docs/provider-oauth.md rule 11). The uses and links are the core's
// (`account_page`), drawn through the controls every page waits the same way with (docs/waiting.md).

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>A switch per use the account's kind offers, as the core decided each.</summary>
    private UIElement UsesGroup(AccountEntry entry, AccountPage page)
    {
        var stack = new StackPanel { Spacing = 8 };
        foreach (var use in page.Uses)
        {
            stack.Children.Add(UseRow(entry, use));
        }
        return Group(L10n.SettingsAccountUsesHeading(), L10n.SettingsAccountUsesDescription(), stack);
    }

    private StackPanel UseRow(AccountEntry entry, AccountUseSwitch use)
    {
        var capability = use.Capability;
        var row = new StackPanel { Spacing = 2 };
        void Set(bool on) => _model.ChangeAccount(new AccountPageIntent.SetUse(entry.Id, capability, on));
        // Switching off deletes what the device holds of the use, so it is asked first where it
        // holds anything; cancelling puts the switch back.
        var asking = $"use:{entry.Id}:{capability}";
        ControlToggle? toggle = null;
        var confirm = new ConfirmPanel(
            L10n.AccountUseOffTitle(UseName(capability)),
            L10n.AccountUseOffMessage(),
            L10n.ActionSwitchOff(),
            $"AccountUseOff_{capability}",
            yes =>
            {
                _confirmOpen = null;
                if (yes)
                {
                    Set(false);
                }
                else
                {
                    toggle?.Restore();
                }
            });
        toggle = new ControlToggle(
            UseName(capability), AccountUseText.Note(use), use.On, use.Control, $"AccountUse_{capability}", on =>
            {
                if (_rebuilding)
                {
                    return;
                }
                if (on && use.AsksProvider)
                {
                    // Chosen, and waiting on the provider: switching it on asks again.
                    _ = SignInAgainFromPageAsync(entry, [capability]);
                }
                else if (!on && use.ConfirmsOff)
                {
                    _confirmOpen = asking;
                    toggle!.Hold();
                    confirm.Show();
                }
                else
                {
                    Set(on);
                }
            });
        // Still asked while the use is still on to be switched off; Switch off waits while another
        // change runs.
        if (_confirmOpen == asking && use.On && use.ConfirmsOff)
        {
            toggle.HoldOff();
            confirm.Show();
        }
        else if (_confirmOpen == asking)
        {
            _confirmOpen = null;
        }
        confirm.Enabled = use.Control.Enabled;
        row.Children.Add(toggle.Element);
        // Both open something and start nothing, so they are only held while a change runs.
        var held = use.Control with { Running = false };
        if (SeparateServerAction(entry, capability, held) is { } separate)
        {
            row.Children.Add(separate);
        }
        if (ServerChangeAction(entry, capability, use.ReviewsServerChange, held) is { } review)
        {
            row.Children.Add(review);
        }
        row.Children.Add(confirm.Element);
        return row;
    }

    /// <summary>A picker per link slot the account can hold.</summary>
    private UIElement LinksGroup(AccountEntry entry, IReadOnlyList<AccountLinkPicker> pickers)
    {
        var stack = new StackPanel { Spacing = 12 };
        foreach (var picker in pickers)
        {
            stack.Children.Add(LinkRow(entry, picker));
        }
        return Group(L10n.SettingsAccountLinksHeading(), L10n.SettingsAccountLinksDescription(), stack);
    }

    private StackPanel LinkRow(AccountEntry entry, AccountLinkPicker picker)
    {
        void Link(string? target)
        {
            if (!_rebuilding)
            {
                _model.ChangeAccount(new AccountPageIntent.SetLink(entry.Id, picker.Slot, target));
            }
        }
        var row = new StackPanel { Spacing = 4 };
        var labels = new List<string> { L10n.SettingsAccountLinkNone() };
        labels.AddRange(picker.Options.Select(option => AccountNameText.Of(option)));
        row.Children.Add(ControlComponents.ComboBox(
            LinkTitle(picker.Slot),
            labels,
            picker.Selected is { } selected ? (int)selected + 1 : 0,
            picker.Control,
            $"AccountLink_{picker.Slot}",
            at => Link(at == 0 ? null : picker.Options[at - 1].Id)));
        // A suggestion is named under the picker, with a button that links it: the person confirms
        // it, it is never linked for them (docs/accounts.md rule 12).
        if (picker.Suggested is { } suggested)
        {
            var option = picker.Options[(int)suggested];
            var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            var text = Description(L10n.SettingsAccountLinkSuggested(AccountNameText.Of(option)));
            text.VerticalAlignment = VerticalAlignment.Center;
            line.Children.Add(text);
            // The picker carries the ring while its change runs; the button beside it is held.
            line.Children.Add(ControlComponents.Button(
                L10n.ActionLink(), picker.Control with { Running = false }, $"AccountLinkSuggested_{picker.Slot}",
                () => Link(option.Id)));
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

    /// <summary>"Sign in again", for an account that signs in at its provider's page: the remedy
    /// for an expired sign-in and for a permission the provider withheld.</summary>
    private UIElement SignInGroup(AccountEntry entry, bool expired)
    {
        var description = expired
            ? L10n.SigninExpiredPrompt(AccountNameText.Of(entry))
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
        SayOnAccounts(null);
        var ended = await _model.SignInAgainAsync(entry.Id, adding, step => SaySignInStep(entry, step));
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
